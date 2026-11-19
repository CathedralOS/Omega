// Reserved toolchain mount recognition and exact captured-path normalization.
// Authored imports select logical names; no filename-prefix lookup lives here.

use diagnostics::Diagnostic;
use std::path::PathBuf;

pub(super) fn is_bundled_omega_path(path: &[String]) -> bool {
    path.first()
        .is_some_and(|segment| segment.as_str() == "omega")
        && path
            .get(1)
            .is_some_and(|segment| segment.as_str() == "language")
}

pub(super) fn is_bundled_core_path(path: &[String]) -> bool {
    is_bundled_omega_path(path)
        && path
            .get(2)
            .is_some_and(|segment| segment.as_str() == "core")
}

pub(super) fn identifier_path_text(path: &[String]) -> String {
    path.join("::")
}

pub(crate) fn bundled_omega_root() -> Result<PathBuf, Vec<Diagnostic>> {
    crate::source::toolchain_library::root().map_err(|error| vec![Diagnostic::error(error)])
}

pub fn bundled_core_root() -> Result<PathBuf, Vec<Diagnostic>> {
    Ok(bundled_omega_root()?.join("core"))
}
