// Reserved toolchain mount recognition and exact captured-path normalization.
// Authored imports select logical names; no filename-prefix lookup lives here.

use diagnostics::Diagnostic;
use std::path::PathBuf;

pub(crate) fn bundled_omega_root() -> Result<PathBuf, Vec<Diagnostic>> {
    crate::source::toolchain_library::root().map_err(|error| vec![Diagnostic::error(error)])
}

pub fn bundled_core_root() -> Result<PathBuf, Vec<Diagnostic>> {
    Ok(bundled_omega_root()?.join("core"))
}

pub(super) fn bundled_build_root() -> Result<PathBuf, Vec<Diagnostic>> {
    Ok(bundled_omega_root()?.join("build"))
}
