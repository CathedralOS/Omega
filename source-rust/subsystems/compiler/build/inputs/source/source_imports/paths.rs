// Reserved toolchain mount recognition and exact captured-path normalization.
// Authored imports select logical names; no filename-prefix lookup lives here.

use diagnostics::Diagnostic;
use std::path::{Path, PathBuf};

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

pub(super) fn bundled_build_root() -> Result<PathBuf, Vec<Diagnostic>> {
    Ok(bundled_omega_root()?.join("build"))
}

/// The entry contract is issued from the installed standard library without
/// executing its Build. Its own package name selects that registered root, just
/// as PackageCompilationInputs::package_name does for a reconciled package.
pub(super) fn contract_package_prefix(root: &Path, first: &str) -> Result<usize, Vec<Diagnostic>> {
    if first != super::entry_contract_seed::ENTRY_PACKAGE {
        return Ok(0);
    }
    Ok(usize::from(
        root == super::entry_contract_seed::entry_contract_root()?,
    ))
}

#[cfg(test)]
mod contract_package_names {
    use super::*;

    #[test]
    fn only_the_installed_standard_library_owns_its_name() {
        let system = bundled_omega_root().unwrap().join("system");
        assert_eq!(contract_package_prefix(&system, "system").unwrap(), 1);
        assert_eq!(contract_package_prefix(&system, "unrelated").unwrap(), 0);
        let other = system.join("otherwise_identically_named_package");
        assert_eq!(contract_package_prefix(&other, "system").unwrap(), 0);
    }
}
