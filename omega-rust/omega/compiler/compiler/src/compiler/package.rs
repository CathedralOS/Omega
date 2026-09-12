use super::*;

/// Continue one package-manager-owned checked production to a retained
/// Terminal report without rerunning its build machine or source discovery.
pub fn retained_terminal_report_from_checked_package(
    root_path: std::path::PathBuf,
    checked: crate::CheckedCompilation,
    profile: proof_admission::AdmissionProfile,
) -> Result<CompileReport, Vec<Diagnostic>> {
    execution::run_on_compile_thread(move || {
        super::compile_package_terminal_report(root_path, checked, &profile)
    })
}
