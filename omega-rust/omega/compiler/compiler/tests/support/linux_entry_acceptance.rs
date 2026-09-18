//! Explicit fixture acceptance of the checked, package-owned entry schema.
//! The target contract is checked as dependency source before the application's
//! entry is selected; application discovery alone cannot authorize that role.

use compiler::{CheckedCompileRequest, compile_to_checked};
use diagnostics::Diagnostic;
use package_compilation::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, PackageCompilationInputs,
    PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::path::Path;

pub fn candidate_linux_x86_64_entry_binding(
    standard_library_root: &Path,
    package: PackageKeyIdentity,
) -> Result<AcceptedSemanticBinding, Vec<Diagnostic>> {
    let inputs = PackageCompilationInputs::new_package(
        package,
        vec![PackageSourceBinding::new(
            package,
            "omega-language-std",
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
            &standard_library_root.join("targets/linux_x86_64/entry.omg"),
            Some("linux_x86_64"),
        )
    })?;
    checked
        .candidate_service_binding(
            AcceptedSemanticBindingRole::LinuxX86_64ProgramEntry,
            package,
            "LinuxX86_64Application",
        )
        .map_err(|diagnostic| vec![diagnostic])
}

// Shared across test targets; not every consumer of this module selects the
// Linux ARM64 slot.
#[allow(dead_code)]
pub fn candidate_linux_arm64_entry_binding(
    standard_library_root: &Path,
    package: PackageKeyIdentity,
) -> Result<AcceptedSemanticBinding, Vec<Diagnostic>> {
    let inputs = PackageCompilationInputs::new_package(
        package,
        vec![PackageSourceBinding::new(
            package,
            "omega-language-std",
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
            &standard_library_root.join("targets/linux_arm64/entry.omg"),
            Some("linux_arm64"),
        )
    })?;
    checked
        .candidate_service_binding(
            AcceptedSemanticBindingRole::LinuxArm64ProgramEntry,
            package,
            "LinuxArm64Application",
        )
        .map_err(|diagnostic| vec![diagnostic])
}
