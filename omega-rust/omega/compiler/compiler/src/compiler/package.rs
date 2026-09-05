use super::*;

/// Continue one package-manager-owned checked production to a retained
/// Terminal report without rerunning its build machine or source discovery.
pub fn retained_terminal_report_from_checked_package(
    root_path: std::path::PathBuf,
    checked: crate::CheckedCompilation,
    profile: proof_admission::AdmissionProfile,
) -> Result<CompileReport, Vec<Diagnostic>> {
    execution::run_on_compile_thread(move || {
        driver::retained_terminal_report_from_checked_package(root_path, checked, &profile)
    })
}

/// Reconstruct checked trust obligations and optional compiler observations
/// for an already-checked package production.
///
/// Package orchestration uses this before consuming its retained checked root
/// into Terminal/native production. The accepted set remains explicit and the
/// returned settlement grants no package-review authority.
pub fn report_checked_compilation_observations(
    options: &CompileOptions,
    artifact_policy: ArtifactEmissionPolicy,
    accepted_trust_admissions: &[TrustAdmission],
    checked: &crate::CheckedCompilation,
) -> Result<TrustAdmissionSettlement, Vec<Diagnostic>> {
    crate::pipeline::reporting::report_checked_observations(
        crate::pipeline::reporting::CheckedObservationInput {
            options,
            artifact_policy,
            accepted_trust_admissions,
            checked,
        },
    )
}
