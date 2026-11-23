//! PDF/A-2b / PDF/A-3b output, embedded file attachments and
//! Factur-X / ZUGFeRD e-invoices.
//!
//! What a conforming file needs on top of a regular ferropdf PDF
//! (ISO 19005-2/-3, level B):
//!
//! * an XMP metadata stream identifying the part and conformance level,
//!   whose properties match the document information dictionary;
//! * an sRGB output intent (every colour ferropdf writes is DeviceRGB or
//!   DeviceGray);
//! * a file identifier in the trailer;
//! * every font embedded and no reference to the `.notdef` glyph — both
//!   enforced by the PDF writer when [`PdfVariant::is_pdfa`] is true;
//! * for PDF/A-3, embedded files carry a MIME type, a modification date and
//!   an `/AFRelationship`, and are listed in the catalog's `/AF` array.
//!
//! Factur-X 1.0 / ZUGFeRD 2.x add one XML invoice attachment and an XMP
//! extension schema (`fx:` namespace) naming it and its profile.

use ferropdf_core::FerroError;
use pdf_writer::{Date, Filter, Finish, Name, Pdf, Ref, Str, TextStr};

/// The sRGB IEC 61966-2.1 profile used as output intent (CC0, see
/// `assets/icc/README.md`).
const SRGB_ICC: &[u8] = include_bytes!("../assets/icc/sRGB-v4.icc");

const FACTUR_X_NS: &str = "urn:factur-x:pdfa:CrossIndustryDocument:invoice:1p0#";

// =============================================================================
// Options
// =============================================================================

/// Which flavour of PDF to write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PdfVariant {
    /// Plain PDF 1.7.
    #[default]
    Standard,
    /// PDF/A-2b (ISO 19005-2, level B). No attachments.
    PdfA2b,
    /// PDF/A-3b (ISO 19005-3, level B). Allows attachments (Factur-X).
    PdfA3b,
}

impl PdfVariant {
    /// Parse `"pdf/a-3b"`, `"PDF/A-2B"`, `"pdfa-3b"`, `"pdf"`… (case-insensitive).
    pub fn parse(s: &str) -> Result<Self, String> {
        let norm: String = s
            .chars()
            .filter(|c| !matches!(c, '/' | '-' | '_' | ' '))
            .collect::<String>()
            .to_ascii_lowercase();
        match norm.as_str() {
            "" | "pdf" | "standard" | "pdf17" | "pdf1.7" => Ok(Self::Standard),
            "pdfa2b" => Ok(Self::PdfA2b),
            "pdfa3b" => Ok(Self::PdfA3b),
            _ => Err(format!(
                "unknown pdf_variant {s:?}: expected \"pdf/a-2b\" or \"pdf/a-3b\""
            )),
        }
    }

    pub fn is_pdfa(self) -> bool {
        self != Self::Standard
    }

    fn part(self) -> Option<u8> {
        match self {
            Self::Standard => None,
            Self::PdfA2b => Some(2),
            Self::PdfA3b => Some(3),
        }
    }
}

/// How an attachment relates to the document (`/AFRelationship`, PDF 2.0
/// §14.13 / PDF/A-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AfRelationship {
    Source,
    Data,
    Alternative,
    Supplement,
    #[default]
    Unspecified,
}

impl AfRelationship {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_ascii_lowercase().as_str() {
            "source" => Ok(Self::Source),
            "data" => Ok(Self::Data),
            "alternative" => Ok(Self::Alternative),
            "supplement" => Ok(Self::Supplement),
            "unspecified" | "" => Ok(Self::Unspecified),
            _ => Err(format!(
                "unknown relationship {s:?}: expected Source, Data, Alternative, \
                 Supplement or Unspecified"
            )),
        }
    }

    fn name(self) -> &'static [u8] {
        match self {
            Self::Source => b"Source",
            Self::Data => b"Data",
            Self::Alternative => b"Alternative",
            Self::Supplement => b"Supplement",
            Self::Unspecified => b"Unspecified",
        }
    }
}

/// A file embedded in the PDF.
#[derive(Debug, Clone)]
pub struct Attachment {
    /// File name shown by PDF viewers, e.g. `"data.csv"`.
    pub name: String,
    pub data: Vec<u8>,
    /// MIME type; guessed from the extension when `None`.
    pub mime_type: Option<String>,
    pub description: Option<String>,
    pub relationship: AfRelationship,
}

/// Factur-X / ZUGFeRD profile (`fx:ConformanceLevel`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FacturXProfile {
    Minimum,
    BasicWl,
    Basic,
    En16931,
    Extended,
    XRechnung,
}

impl FacturXProfile {
    pub fn parse(s: &str) -> Result<Self, String> {
        let norm: String = s
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        match norm.as_str() {
            "minimum" => Ok(Self::Minimum),
            "basicwl" => Ok(Self::BasicWl),
            "basic" => Ok(Self::Basic),
            "en16931" | "comfort" => Ok(Self::En16931),
            "extended" => Ok(Self::Extended),
            "xrechnung" => Ok(Self::XRechnung),
            _ => Err(format!(
                "unknown Factur-X profile {s:?}: expected MINIMUM, BASIC WL, BASIC, \
                 EN 16931, EXTENDED or XRECHNUNG"
            )),
        }
    }

    /// Detect the profile from the invoice's
    /// `GuidelineSpecifiedDocumentContextParameter/ID`.
    pub fn detect(xml: &str) -> Option<Self> {
        let start = xml.find("GuidelineSpecifiedDocumentContextParameter")?;
        let rest = &xml[start..];
        // The first <…:ID> element inside it.
        let id_open = find_element_start(rest, "ID")?;
        let after = &rest[id_open..];
        let gt = after.find('>')?;
        let value_and_rest = &after[gt + 1..];
        let lt = value_and_rest.find('<')?;
        let id = value_and_rest[..lt].trim().to_ascii_lowercase();
        if id.contains("xrechnung") {
            Some(Self::XRechnung)
        } else if id.contains("factur-x.eu:1p0:minimum") {
            Some(Self::Minimum)
        } else if id.contains("factur-x.eu:1p0:basicwl") {
            Some(Self::BasicWl)
        } else if id.contains("factur-x.eu:1p0:basic") {
            Some(Self::Basic)
        } else if id.contains("factur-x.eu:1p0:extended") {
            Some(Self::Extended)
        } else if id.starts_with("urn:cen.eu:en16931:2017") {
            Some(Self::En16931)
        } else {
            None
        }
    }

    fn xmp_name(self) -> &'static str {
        match self {
            Self::Minimum => "MINIMUM",
            Self::BasicWl => "BASIC WL",
            Self::Basic => "BASIC",
            Self::En16931 => "EN 16931",
            Self::Extended => "EXTENDED",
            Self::XRechnung => "XRECHNUNG",
        }
    }

    /// File name required by the specifications.
    pub fn file_name(self) -> &'static str {
        match self {
            Self::XRechnung => "xrechnung.xml",
            _ => "factur-x.xml",
        }
    }

    /// Factur-X 1.0.07: MINIMUM and BASIC WL are not full invoices, so the
    /// XML is `Data`; the other profiles are an `Alternative` of the PDF.
    fn default_relationship(self) -> AfRelationship {
        match self {
            Self::Minimum | Self::BasicWl => AfRelationship::Data,
            _ => AfRelationship::Alternative,
        }
    }
}

/// Start index of the first `<ID` / `<prefix:ID` start tag in `s`.
fn find_element_start(s: &str, local: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while let Some(p) = s[i..].find('<') {
        let tag_start = i + p + 1;
        let name_end = s[tag_start..]
            .find(|c: char| c == '>' || c == '/' || c.is_whitespace())
            .map(|e| tag_start + e)?;
        let name = &s[tag_start..name_end];
        let local_name = name.rsplit(':').next().unwrap_or(name);
        if local_name == local && bytes.get(tag_start) != Some(&b'/') {
            return Some(i + p);
        }
        i = tag_start;
    }
    None
}

/// A Factur-X / ZUGFeRD XML invoice to embed.
#[derive(Debug, Clone)]
pub struct FacturX {
    /// The CII (`rsm:CrossIndustryInvoice`) XML, UTF-8.
    pub xml: Vec<u8>,
    /// `None` detects it from the XML.
    pub profile: Option<FacturXProfile>,
}

// =============================================================================
// Validation
// =============================================================================

/// Options checked and resolved into what gets written.
pub(crate) struct Plan {
    pub variant: PdfVariant,
    /// PDF/UA-1 (ISO 14289-1): `pdfuaid` in XMP, strict fonts.
    pub ua: bool,
    attachments: Vec<Attachment>,
    factur_x: Option<(FacturXProfile, String)>,
}

impl Plan {
    pub fn new(
        variant: PdfVariant,
        attachments: &[Attachment],
        factur_x: Option<&FacturX>,
    ) -> ferropdf_core::Result<Self> {
        let err = |m: String| FerroError::PdfWrite(m);
        let mut variant = variant;
        let mut files: Vec<Attachment> = attachments.to_vec();
        let mut fx = None;

        if let Some(f) = factur_x {
            match variant {
                PdfVariant::Standard => variant = PdfVariant::PdfA3b,
                PdfVariant::PdfA3b => {}
                PdfVariant::PdfA2b => {
                    return Err(err(
                        "factur_x requires PDF/A-3b (PDF/A-2 cannot embed XML files)".into(),
                    ))
                }
            }
            let xml = std::str::from_utf8(&f.xml)
                .map_err(|_| err("factur_x: the XML invoice must be UTF-8".into()))?;
            if !xml.contains("CrossIndustryInvoice") {
                return Err(err(
                    "factur_x: expected a UN/CEFACT CII invoice (rsm:CrossIndustryInvoice)".into(),
                ));
            }
            let profile = match f.profile {
                Some(p) => p,
                None => FacturXProfile::detect(xml).ok_or_else(|| {
                    err("factur_x: cannot detect the profile from \
                         GuidelineSpecifiedDocumentContextParameter; pass factur_x_profile"
                        .into())
                })?,
            };
            let name = profile.file_name().to_string();
            files.insert(
                0,
                Attachment {
                    name: name.clone(),
                    data: f.xml.clone(),
                    mime_type: Some("text/xml".into()),
                    description: Some(format!("Factur-X invoice ({})", profile.xmp_name())),
                    relationship: profile.default_relationship(),
                },
            );
            fx = Some((profile, name));
        }

        if variant == PdfVariant::PdfA2b && !files.is_empty() {
            return Err(err(
                "PDF/A-2b cannot embed arbitrary files; use pdf_variant=\"pdf/a-3b\"".into(),
            ));
        }
        let mut seen = std::collections::HashSet::new();
        for a in &files {
            if a.name.trim().is_empty() {
                return Err(err("attachment name must not be empty".into()));
            }
            if !seen.insert(a.name.as_str()) {
                return Err(err(format!("duplicate attachment name {:?}", a.name)));
            }
        }

        Ok(Self {
            variant,
            ua: false,
            attachments: files,
            factur_x: fx,
        })
    }

    pub fn is_pdfa(&self) -> bool {
        self.variant.is_pdfa()
    }

    /// PDF/A and PDF/UA: every font embedded, no `.notdef`.
    pub fn strict_fonts(&self) -> bool {
        self.is_pdfa() || self.ua
    }

    /// An XMP metadata stream (and matching info dates) is written.
    pub fn writes_metadata(&self) -> bool {
        self.is_pdfa() || self.ua
    }

    pub fn needs_catalog_entries(&self) -> bool {
        self.writes_metadata() || !self.attachments.is_empty()
    }
}

// =============================================================================
// Writing
// =============================================================================

/// Object numbers reserved before the catalog is written.
pub(crate) struct Refs {
    metadata: Option<Ref>,
    icc: Option<Ref>,
    /// (file spec, embedded file stream) per attachment.
    files: Vec<(Ref, Ref)>,
}

impl Refs {
    pub fn alloc(plan: &Plan, next_ref: &mut dyn FnMut() -> Ref) -> Self {
        let pdfa = plan.is_pdfa();
        Self {
            metadata: plan.writes_metadata().then(&mut *next_ref),
            icc: pdfa.then(&mut *next_ref),
            files: plan
                .attachments
                .iter()
                .map(|_| (next_ref(), next_ref()))
                .collect(),
        }
    }
}

/// Add `/Metadata`, `/OutputIntents`, `/AF` and `/Names /EmbeddedFiles`.
pub(crate) fn write_catalog_entries(
    catalog: &mut pdf_writer::writers::Catalog,
    plan: &Plan,
    refs: &Refs,
) {
    if let Some(m) = refs.metadata {
        catalog.metadata(m);
    }
    if let Some(icc) = refs.icc {
        let mut intents = catalog.insert(Name(b"OutputIntents")).array();
        let mut oi = intents.push().dict();
        oi.pair(Name(b"Type"), Name(b"OutputIntent"));
        oi.pair(Name(b"S"), Name(b"GTS_PDFA1"));
        oi.pair(
            Name(b"OutputConditionIdentifier"),
            TextStr("sRGB IEC61966-2.1"),
        );
        oi.pair(Name(b"RegistryName"), TextStr("http://www.color.org"));
        oi.pair(Name(b"Info"), TextStr("sRGB IEC61966-2.1"));
        oi.pair(Name(b"DestOutputProfile"), icc);
        oi.finish();
        intents.finish();
    }
    if !plan.attachments.is_empty() {
        catalog
            .insert(Name(b"AF"))
            .array()
            .items(refs.files.iter().map(|(spec, _)| *spec));
        // Name tree keys must be sorted (PDF 32000 §7.9.6).
        let mut entries: Vec<(&str, Ref)> = plan
            .attachments
            .iter()
            .zip(&refs.files)
            .map(|(a, (spec, _))| (a.name.as_str(), *spec))
            .collect();
        entries.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        let mut names = catalog.names();
        let mut tree = names.embedded_files();
        let mut leaf = tree.names();
        for (name, spec) in entries {
            leaf.insert(Str(name.as_bytes()), spec);
        }
    }
}

/// A UTC timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Timestamp {
    year: i64,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
}

impl Timestamp {
    /// Now, or `SOURCE_DATE_EPOCH` when set (reproducible builds).
    pub fn now() -> Self {
        let secs = std::env::var("SOURCE_DATE_EPOCH")
            .ok()
            .and_then(|v| v.trim().parse::<i64>().ok())
            .unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0)
            });
        Self::from_unix(secs)
    }

    pub fn from_unix(secs: i64) -> Self {
        let days = secs.div_euclid(86_400);
        let rem = secs.rem_euclid(86_400);
        // Howard Hinnant's civil_from_days.
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
        let year = yoe + era * 400 + i64::from(month <= 2);
        Self {
            year: year.clamp(0, 9999),
            month,
            day,
            hour: (rem / 3600) as u8,
            minute: (rem % 3600 / 60) as u8,
            second: (rem % 60) as u8,
        }
    }

    pub fn pdf_date(self) -> Date {
        Date::new(self.year as u16)
            .month(self.month)
            .day(self.day)
            .hour(self.hour)
            .minute(self.minute)
            .second(self.second)
            .utc_offset_hour(0)
            .utc_offset_minute(0)
    }

    fn xmp(self) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

/// Write the XMP packet, the ICC profile and the attachments.
pub(crate) fn write_objects(
    pdf: &mut Pdf,
    plan: &Plan,
    refs: &Refs,
    title: Option<&str>,
    author: Option<&str>,
    producer: &str,
    now: Timestamp,
) -> ferropdf_core::Result<()> {
    if let Some(m) = refs.metadata {
        let xmp = build_xmp(plan, title, author, producer, now);
        // Uncompressed so archive tools can read it without a PDF parser.
        pdf.metadata(m, xmp.as_bytes()).finish();
    }
    if let Some(icc) = refs.icc {
        let compressed = crate::font_subsetter::compress_data(SRGB_ICC)?;
        let mut stream = pdf.icc_profile(icc, &compressed);
        stream.n(3);
        stream.alternate_name(Name(b"DeviceRGB"));
        stream.filter(Filter::FlateDecode);
        stream.finish();
    }
    for (a, (spec_ref, file_ref)) in plan.attachments.iter().zip(&refs.files) {
        let compressed = crate::font_subsetter::compress_data(&a.data)?;
        let mime = a
            .mime_type
            .clone()
            .unwrap_or_else(|| guess_mime(&a.name).to_string());
        let mut ef = pdf.embedded_file(*file_ref, &compressed);
        ef.filter(Filter::FlateDecode);
        ef.subtype(Name(mime.as_bytes()));
        ef.params()
            .size(a.data.len().min(i32::MAX as usize) as i32)
            .modification_date(now.pdf_date());
        ef.finish();

        let mut fs = pdf.file_spec(*spec_ref);
        fs.path(Str(ascii_file_name(&a.name).as_bytes()));
        fs.unic_file(TextStr(&a.name));
        if let Some(d) = &a.description {
            fs.description(TextStr(d));
        }
        fs.pair(Name(b"AFRelationship"), Name(a.relationship.name()));
        let mut efd = fs.insert(Name(b"EF")).dict();
        efd.pair(Name(b"F"), *file_ref);
        efd.pair(Name(b"UF"), *file_ref);
        efd.finish();
        fs.finish();
    }
    Ok(())
}

/// Trailer `/ID`: a digest of the document (ISO 19005-2 §6.1.3).
pub(crate) fn file_id(seed: &[&[u8]]) -> Vec<u8> {
    use std::hash::{Hash, Hasher};
    let mut out = Vec::with_capacity(16);
    for salt in [0x9e37_79b9_u64, 0x85eb_ca6b] {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        salt.hash(&mut h);
        seed.hash(&mut h);
        out.extend_from_slice(&Hasher::finish(&h).to_be_bytes());
    }
    out
}

fn guess_mime(name: &str) -> &'static str {
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "xml" => "text/xml",
        "csv" => "text/csv",
        "txt" => "text/plain",
        "html" | "htm" => "text/html",
        "json" => "application/json",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        _ => "application/octet-stream",
    }
}

/// `/F` is a byte string: keep printable ASCII, replace the rest.
fn ascii_file_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_graphic() || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            // Characters XML 1.0 cannot carry at all.
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => out.push(c),
        }
    }
    out
}

fn build_xmp(
    plan: &Plan,
    title: Option<&str>,
    author: Option<&str>,
    producer: &str,
    now: Timestamp,
) -> String {
    let date = now.xmp();
    let mut x = String::with_capacity(4096);
    x.push_str("<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n");
    x.push_str("<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n");
    x.push_str("<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n");

    if let Some(part) = plan.variant.part() {
        x.push_str(&format!(
            "<rdf:Description rdf:about=\"\" xmlns:pdfaid=\"http://www.aiim.org/pdfa/ns/id/\">\n\
             <pdfaid:part>{part}</pdfaid:part>\n\
             <pdfaid:conformance>B</pdfaid:conformance>\n\
             </rdf:Description>\n"
        ));
    }

    if plan.ua {
        x.push_str(
            "<rdf:Description rdf:about=\"\" xmlns:pdfuaid=\"http://www.aiim.org/pdfua/ns/id/\">\n\
             <pdfuaid:part>1</pdfuaid:part>\n\
             </rdf:Description>\n",
        );
    }

    x.push_str(
        "<rdf:Description rdf:about=\"\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n\
         <dc:format>application/pdf</dc:format>\n",
    );
    if let Some(t) = title {
        x.push_str(&format!(
            "<dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">{}</rdf:li></rdf:Alt></dc:title>\n",
            xml_escape(t)
        ));
    }
    if let Some(a) = author {
        x.push_str(&format!(
            "<dc:creator><rdf:Seq><rdf:li>{}</rdf:li></rdf:Seq></dc:creator>\n",
            xml_escape(a)
        ));
    }
    x.push_str("</rdf:Description>\n");

    x.push_str(&format!(
        "<rdf:Description rdf:about=\"\" xmlns:pdf=\"http://ns.adobe.com/pdf/1.3/\">\n\
         <pdf:Producer>{}</pdf:Producer>\n\
         </rdf:Description>\n",
        xml_escape(producer)
    ));
    x.push_str(&format!(
        "<rdf:Description rdf:about=\"\" xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\">\n\
         <xmp:CreateDate>{date}</xmp:CreateDate>\n\
         <xmp:ModifyDate>{date}</xmp:ModifyDate>\n\
         <xmp:MetadataDate>{date}</xmp:MetadataDate>\n\
         </rdf:Description>\n"
    ));

    if let Some((profile, file_name)) = &plan.factur_x {
        x.push_str(&format!(
            "<rdf:Description rdf:about=\"\" xmlns:fx=\"{FACTUR_X_NS}\">\n\
             <fx:DocumentType>INVOICE</fx:DocumentType>\n\
             <fx:DocumentFileName>{file_name}</fx:DocumentFileName>\n\
             <fx:Version>1.0</fx:Version>\n\
             <fx:ConformanceLevel>{}</fx:ConformanceLevel>\n\
             </rdf:Description>\n",
            profile.xmp_name()
        ));
    }

    // PDF/A requires a schema description for every non-standard namespace
    // used in XMP (ISO 19005-2 §6.6.2.3.3): fx (Factur-X) and pdfuaid.
    type Prop = (&'static str, &'static str, &'static str, &'static str);
    let mut schemas: Vec<(&str, &str, &str, Vec<Prop>)> = Vec::new();
    if plan.factur_x.is_some() {
        schemas.push((
            "Factur-X PDFA Extension Schema",
            FACTUR_X_NS,
            "fx",
            vec![
                (
                    "DocumentFileName",
                    "Text",
                    "external",
                    "The name of the embedded XML document",
                ),
                (
                    "DocumentType",
                    "Text",
                    "external",
                    "The type of the hybrid document in capital letters, e.g. INVOICE or ORDER",
                ),
                (
                    "Version",
                    "Text",
                    "external",
                    "The actual version of the standard applying to the embedded XML document",
                ),
                (
                    "ConformanceLevel",
                    "Text",
                    "external",
                    "The conformance level of the embedded XML document",
                ),
            ],
        ));
    }
    if plan.ua && plan.is_pdfa() {
        schemas.push((
            "PDF/UA Universal Accessibility Schema",
            "http://www.aiim.org/pdfua/ns/id/",
            "pdfuaid",
            vec![(
                "part",
                "Integer",
                "internal",
                "Indicates, which part of ISO 14289 standard is followed",
            )],
        ));
    }
    if !schemas.is_empty() {
        x.push_str(
            "<rdf:Description rdf:about=\"\" \
             xmlns:pdfaExtension=\"http://www.aiim.org/pdfa/ns/extension/\" \
             xmlns:pdfaSchema=\"http://www.aiim.org/pdfa/ns/schema#\" \
             xmlns:pdfaProperty=\"http://www.aiim.org/pdfa/ns/property#\">\n\
             <pdfaExtension:schemas><rdf:Bag>\n",
        );
        for (schema, ns, prefix, props) in schemas {
            x.push_str(&format!(
                "<rdf:li rdf:parseType=\"Resource\">\n\
                 <pdfaSchema:schema>{schema}</pdfaSchema:schema>\n\
                 <pdfaSchema:namespaceURI>{ns}</pdfaSchema:namespaceURI>\n\
                 <pdfaSchema:prefix>{prefix}</pdfaSchema:prefix>\n\
                 <pdfaSchema:property><rdf:Seq>\n"
            ));
            for (name, value_type, category, desc) in props {
                x.push_str(&format!(
                    "<rdf:li rdf:parseType=\"Resource\">\
                     <pdfaProperty:name>{name}</pdfaProperty:name>\
                     <pdfaProperty:valueType>{value_type}</pdfaProperty:valueType>\
                     <pdfaProperty:category>{category}</pdfaProperty:category>\
                     <pdfaProperty:description>{desc}</pdfaProperty:description>\
                     </rdf:li>\n"
                ));
            }
            x.push_str("</rdf:Seq></pdfaSchema:property>\n</rdf:li>\n");
        }
        x.push_str("</rdf:Bag></pdfaExtension:schemas>\n</rdf:Description>\n");
    }

    x.push_str("</rdf:RDF>\n</x:xmpmeta>\n");
    // Padding lets tools update the packet in place.
    for _ in 0..20 {
        x.push_str(&" ".repeat(99));
        x.push('\n');
    }
    x.push_str("<?xpacket end=\"w\"?>");
    x
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_variants() {
        assert_eq!(PdfVariant::parse("pdf/a-3b").unwrap(), PdfVariant::PdfA3b);
        assert_eq!(PdfVariant::parse("PDF/A-2B").unwrap(), PdfVariant::PdfA2b);
        assert_eq!(PdfVariant::parse("pdfa_3b").unwrap(), PdfVariant::PdfA3b);
        assert_eq!(PdfVariant::parse("pdf").unwrap(), PdfVariant::Standard);
        assert!(PdfVariant::parse("pdf/a-1a").is_err());
    }

    #[test]
    fn detects_factur_x_profiles() {
        let doc = |id: &str| {
            format!(
                "<rsm:CrossIndustryInvoice><rsm:ExchangedDocumentContext>\
                 <ram:GuidelineSpecifiedDocumentContextParameter>\n  <ram:ID>{id}</ram:ID>\
                 </ram:GuidelineSpecifiedDocumentContextParameter>"
            )
        };
        let cases = [
            ("urn:factur-x.eu:1p0:minimum", FacturXProfile::Minimum),
            ("urn:factur-x.eu:1p0:basicwl", FacturXProfile::BasicWl),
            (
                "urn:cen.eu:en16931:2017#compliant#urn:factur-x.eu:1p0:basic",
                FacturXProfile::Basic,
            ),
            ("urn:cen.eu:en16931:2017", FacturXProfile::En16931),
            (
                "urn:cen.eu:en16931:2017#conformant#urn:factur-x.eu:1p0:extended",
                FacturXProfile::Extended,
            ),
            (
                "urn:cen.eu:en16931:2017#compliant#urn:xeinkauf.de:kosit:xrechnung_3.0",
                FacturXProfile::XRechnung,
            ),
        ];
        for (id, want) in cases {
            assert_eq!(FacturXProfile::detect(&doc(id)), Some(want), "{id}");
        }
        assert_eq!(FacturXProfile::detect("<x/>"), None);
        assert_eq!(FacturXProfile::detect(&doc("urn:example")), None);
    }

    #[test]
    fn parses_profile_names() {
        assert_eq!(
            FacturXProfile::parse("EN 16931").unwrap(),
            FacturXProfile::En16931
        );
        assert_eq!(
            FacturXProfile::parse("basic-wl").unwrap(),
            FacturXProfile::BasicWl
        );
        assert!(FacturXProfile::parse("gold").is_err());
    }

    #[test]
    fn unix_time_to_civil_date() {
        let t = Timestamp::from_unix(0);
        assert_eq!((t.year, t.month, t.day, t.hour), (1970, 1, 1, 0));
        // 2024-02-29T12:34:56Z (leap day)
        let t = Timestamp::from_unix(1_709_210_096);
        assert_eq!(
            (t.year, t.month, t.day, t.hour, t.minute, t.second),
            (2024, 2, 29, 12, 34, 56)
        );
        assert_eq!(t.xmp(), "2024-02-29T12:34:56Z");
    }

    #[test]
    fn factur_x_forces_pdfa3_and_rejects_pdfa2() {
        let fx = FacturX {
            xml: b"<rsm:CrossIndustryInvoice/>".to_vec(),
            profile: Some(FacturXProfile::En16931),
        };
        let plan = Plan::new(PdfVariant::Standard, &[], Some(&fx)).unwrap();
        assert_eq!(plan.variant, PdfVariant::PdfA3b);
        assert_eq!(plan.attachments[0].name, "factur-x.xml");
        assert_eq!(
            plan.attachments[0].relationship,
            AfRelationship::Alternative
        );
        assert!(Plan::new(PdfVariant::PdfA2b, &[], Some(&fx)).is_err());
    }

    #[test]
    fn rejects_bad_attachments() {
        let a = |n: &str| Attachment {
            name: n.into(),
            data: vec![1],
            mime_type: None,
            description: None,
            relationship: AfRelationship::Unspecified,
        };
        assert!(Plan::new(PdfVariant::PdfA3b, &[a("x"), a("x")], None).is_err());
        assert!(Plan::new(PdfVariant::PdfA3b, &[a(" ")], None).is_err());
        assert!(Plan::new(PdfVariant::PdfA2b, &[a("x")], None).is_err());
        assert!(Plan::new(PdfVariant::Standard, &[a("x")], None).is_ok());
        let fx = FacturX {
            xml: b"<rsm:CrossIndustryInvoice/>".to_vec(),
            profile: Some(FacturXProfile::Basic),
        };
        assert!(Plan::new(PdfVariant::PdfA3b, &[a("factur-x.xml")], Some(&fx)).is_err());
        // Not an invoice / undetectable profile.
        let bad = FacturX {
            xml: b"<Invoice/>".to_vec(),
            profile: None,
        };
        assert!(Plan::new(PdfVariant::PdfA3b, &[], Some(&bad)).is_err());
    }

    #[test]
    fn xmp_has_identification_and_escapes() {
        let fx = FacturX {
            xml: b"<rsm:CrossIndustryInvoice/>".to_vec(),
            profile: Some(FacturXProfile::BasicWl),
        };
        let plan = Plan::new(PdfVariant::Standard, &[], Some(&fx)).unwrap();
        let x = build_xmp(
            &plan,
            Some("Facture <42> & co"),
            Some("ACME"),
            "ferropdf",
            Timestamp::from_unix(0),
        );
        assert!(x.contains("<pdfaid:part>3</pdfaid:part>"));
        assert!(x.contains("<pdfaid:conformance>B</pdfaid:conformance>"));
        assert!(x.contains("Facture &lt;42&gt; &amp; co"));
        assert!(x.contains("<fx:ConformanceLevel>BASIC WL</fx:ConformanceLevel>"));
        assert!(x.contains("<pdfaSchema:prefix>fx</pdfaSchema:prefix>"));
        assert!(x.contains("<xmp:CreateDate>1970-01-01T00:00:00Z</xmp:CreateDate>"));
        assert!(x.ends_with("<?xpacket end=\"w\"?>"));
    }

    #[test]
    fn guesses_mime_types() {
        assert_eq!(guess_mime("a.XML"), "text/xml");
        assert_eq!(guess_mime("report.csv"), "text/csv");
        assert_eq!(guess_mime("blob"), "application/octet-stream");
    }

    #[test]
    fn icc_profile_is_rgb_display() {
        assert_eq!(&SRGB_ICC[12..16], b"mntr");
        assert_eq!(&SRGB_ICC[16..20], b"RGB ");
        assert_eq!(
            u32::from_be_bytes(SRGB_ICC[0..4].try_into().unwrap()) as usize,
            SRGB_ICC.len()
        );
    }
}
