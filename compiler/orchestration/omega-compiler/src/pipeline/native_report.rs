use super::artifacts::ArtifactWriter;

use omega_artifacts::NativeSurfaceReport;
use omega_backend_plan::NativePlan;
use omega_core::diagnostics::Diagnostic;

pub(crate) fn write_native_report(
    artifacts: &ArtifactWriter,
    native_surface: &NativeSurfaceReport,
    native_plan: &NativePlan,
) -> Result<(), Diagnostic> {
    let output = omega_native_report::native_report_text(native_surface, native_plan);
    artifacts.write_text("09_native_plan.txt", &output)
}
