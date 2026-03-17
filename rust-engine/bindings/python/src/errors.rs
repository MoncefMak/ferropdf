//! Error conversion helpers — map FerroError to PyO3 exceptions.

use pyo3::prelude::*;
use ferropdf_core::FerroError;

/// Convert a `FerroError` into a `PyRuntimeError`.
pub fn to_py_err(e: FerroError) -> PyErr {
    pyo3::exceptions::PyRuntimeError::new_err(e.to_string())
}
