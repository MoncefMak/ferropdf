//! Path-resolution sandbox for local resources (`<img>`, `<link>`, `@font-face`).
//!
//! The render pipeline trusts callers to provide a `base_url` that names the
//! directory inside which user-supplied HTML may reach for assets. Without it,
//! every `src`/`href`/`url()` that points at a local file is refused so that
//! adversarial HTML can't read the host filesystem.
//!
//! When `base_url` is set, paths are resolved relative to it, then canonicalized
//! and verified to remain under the canonical `base_url`. This blocks
//! `../../etc/passwd`-style traversal as well as absolute paths that would
//! otherwise replace the base via `Path::join`.

use std::path::PathBuf;

/// Maximum bytes a single asset (image, font, stylesheet) may consume.
pub const MAX_RESOURCE_BYTES: u64 = 50 * 1024 * 1024;

/// Resolution outcome.
#[derive(Debug)]
pub enum ResolveError {
    /// `src` named an `http(s)://` URL — outbound fetches are disabled.
    HttpDisabled,
    /// `base_url` was `None`, so no local path can be resolved.
    NoBaseUrl,
    /// `base_url` was set but did not exist or could not be canonicalized.
    BadBaseUrl(String),
    /// The resolved path didn't exist or could not be canonicalized.
    NotFound(String),
    /// The resolved path was outside `base_url` (path traversal).
    Escapes,
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolveError::HttpDisabled => {
                write!(f, "HTTP fetch disabled — ferropdf does not load remote URLs")
            }
            ResolveError::NoBaseUrl => write!(
                f,
                "no base_url set; local filesystem reads disabled — set Options(base_url=...) to enable"
            ),
            ResolveError::BadBaseUrl(s) => write!(f, "base_url cannot be canonicalized: {}", s),
            ResolveError::NotFound(s) => write!(f, "path not found: {}", s),
            ResolveError::Escapes => write!(f, "path resolves outside base_url"),
        }
    }
}

/// Resolve `src` (an `<img src>`, `<link href>`, or `@font-face url()` value)
/// against `base_url` with sandbox checks.
///
/// Returns the canonicalized absolute path on success, or a structured error
/// describing why the resource is refused. Callers convert the error into a
/// `RenderWarning::ImageLoadFailed` / `StylesheetFailed` so the user sees what
/// happened without the render being aborted.
pub fn resolve_local_path(src: &str, base_url: Option<&str>) -> Result<PathBuf, ResolveError> {
    if src.starts_with("http://") || src.starts_with("https://") {
        return Err(ResolveError::HttpDisabled);
    }

    let src = src
        .strip_prefix("file:///")
        .map(|s| format!("/{}", s))
        .unwrap_or_else(|| src.strip_prefix("file://").unwrap_or(src).to_string());
    let src = src.trim();

    let base = base_url.ok_or(ResolveError::NoBaseUrl)?;
    let base_path = std::path::Path::new(base);
    let base_dir = if base_path.is_file() {
        base_path.parent().unwrap_or(base_path)
    } else {
        base_path
    };

    let base_canonical = std::fs::canonicalize(base_dir)
        .map_err(|e| ResolveError::BadBaseUrl(format!("{}: {}", base_dir.display(), e)))?;

    // Reject absolute src — `Path::join(absolute)` silently replaces the base
    // and would defeat the sandbox. Callers must use paths relative to base_url.
    if std::path::Path::new(src).is_absolute() {
        return Err(ResolveError::Escapes);
    }

    let joined = base_dir.join(src);

    // canonicalize requires the file to exist, which is the right behavior:
    // we should refuse to read a file we cannot prove lives in the sandbox.
    let canonical = std::fs::canonicalize(&joined)
        .map_err(|_| ResolveError::NotFound(joined.display().to_string()))?;

    if !canonical.starts_with(&base_canonical) {
        return Err(ResolveError::Escapes);
    }

    Ok(canonical)
}

/// Read a sandboxed file with a hard size cap.
pub fn read_sandboxed(src: &str, base_url: Option<&str>) -> Result<Vec<u8>, String> {
    let path = resolve_local_path(src, base_url).map_err(|e| e.to_string())?;
    let metadata =
        std::fs::metadata(&path).map_err(|e| format!("metadata for {}: {}", path.display(), e))?;
    if metadata.len() > MAX_RESOURCE_BYTES {
        return Err(format!(
            "resource exceeds {} byte cap ({} bytes)",
            MAX_RESOURCE_BYTES,
            metadata.len()
        ));
    }
    std::fs::read(&path).map_err(|e| format!("read {}: {}", path.display(), e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    fn tmpdir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn write_file(dir: &std::path::Path, name: &str, content: &[u8]) -> PathBuf {
        let p = dir.join(name);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut f = fs::File::create(&p).unwrap();
        f.write_all(content).unwrap();
        p
    }

    #[test]
    fn rejects_http() {
        let err = resolve_local_path("http://attacker/x.png", Some("/tmp")).unwrap_err();
        assert!(matches!(err, ResolveError::HttpDisabled));
    }

    #[test]
    fn rejects_https() {
        let err = resolve_local_path("https://attacker/x.png", Some("/tmp")).unwrap_err();
        assert!(matches!(err, ResolveError::HttpDisabled));
    }

    #[test]
    fn rejects_local_path_without_base_url() {
        let err = resolve_local_path("/etc/passwd", None).unwrap_err();
        assert!(matches!(err, ResolveError::NoBaseUrl));
    }

    #[test]
    fn rejects_relative_path_without_base_url() {
        let err = resolve_local_path("logo.png", None).unwrap_err();
        assert!(matches!(err, ResolveError::NoBaseUrl));
    }

    #[test]
    fn rejects_absolute_path_with_base_url() {
        let dir = tmpdir();
        write_file(dir.path(), "logo.png", b"fake");
        let err =
            resolve_local_path("/etc/passwd", Some(dir.path().to_str().unwrap())).unwrap_err();
        assert!(matches!(err, ResolveError::Escapes));
    }

    #[test]
    fn allows_relative_path_under_base_url() {
        let dir = tmpdir();
        let written = write_file(dir.path(), "logo.png", b"fake");
        let canonical_written = std::fs::canonicalize(&written).unwrap();
        let resolved = resolve_local_path("logo.png", Some(dir.path().to_str().unwrap())).unwrap();
        assert_eq!(resolved, canonical_written);
    }

    #[test]
    fn rejects_traversal_above_base_url() {
        let outer = tmpdir();
        let inner = outer.path().join("inner");
        std::fs::create_dir(&inner).unwrap();
        write_file(outer.path(), "secret.txt", b"top-secret");
        let err = resolve_local_path("../secret.txt", Some(inner.to_str().unwrap())).unwrap_err();
        assert!(matches!(err, ResolveError::Escapes), "got {:?}", err);
    }

    #[test]
    fn rejects_oversize_resource() {
        let dir = tmpdir();
        let big = vec![0u8; (MAX_RESOURCE_BYTES + 1) as usize];
        write_file(dir.path(), "big.bin", &big);
        let err = read_sandboxed("big.bin", Some(dir.path().to_str().unwrap())).unwrap_err();
        assert!(err.contains("byte cap"), "got {}", err);
    }

    #[test]
    fn missing_file_yields_not_found() {
        let dir = tmpdir();
        let err = resolve_local_path("nope.png", Some(dir.path().to_str().unwrap())).unwrap_err();
        assert!(matches!(err, ResolveError::NotFound(_)));
    }
}
