//! Explicit fixture acceptance of the checked, package-owned UEFI entry schema.
//! Same review posture as the macOS entry acceptance: the target contract is
//! checked as dependency source before the application's entry is selected.

use compiler::{CheckedCompileRequest, compile_to_checked};
use diagnostics::Diagnostic;
use package_compilation::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, PackageCompilationInputs,
    PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::path::Path;

pub fn candidate_uefi_entry_binding(
    standard_library_root: &Path,
    package: PackageKeyIdentity,
) -> Result<AcceptedSemanticBinding, Vec<Diagnostic>> {
    let inputs = PackageCompilationInputs::new_package(
        package,
        vec![PackageSourceBinding::new(
            package,
            "omega_language_std",
            standard_library_root.to_path_buf(),
        )],
        Vec::new(),
    )
    .map_err(|errors| {
        vec![Diagnostic::error(format!(
            "entry fixture inputs: {errors:?}"
        ))]
    })?;
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(
            &standard_library_root.join("targets/uefi_x86_64/entry.omg"),
            Some("uefi_x86_64"),
        )
    })?;
    checked
        .candidate_service_binding(
            AcceptedSemanticBindingRole::UefiX64ProgramEntry,
            package,
            "UefiApplication",
        )
        .map_err(|diagnostic| vec![diagnostic])
}
