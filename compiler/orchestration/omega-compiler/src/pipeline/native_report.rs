use super::artifacts::ArtifactWriter;

use omega_artifacts::NativeSurfaceReport;
use omega_backend_plan::NativePlan;
use omega_backend_report::{BackendReportInput, BackendReportPhaseTiming};
use omega_core::diagnostics::Diagnostic;

pub(crate) fn write_native_report(
    artifacts: &ArtifactWriter,
    native_surface: &NativeSurfaceReport,
    native_plan: &NativePlan,
) -> Result<(), Diagnostic> {
    let phase_timings = native_plan
        .phase_timings
        .iter()
        .map(|timing| BackendReportPhaseTiming {
            phase: timing.phase.clone(),
            microseconds: timing.microseconds,
            allocations: timing.allocations,
        })
        .collect::<Vec<_>>();
    let report_input = BackendReportInput {
        target: native_plan.target,
        entry_key: native_plan.entry_key,
        phase_timings: &phase_timings,
        host_abi: &native_plan.host_abi,
        host_calls: &native_plan.host_calls,
        state_calls: &native_plan.state_calls,
        alias_flow: &native_plan.alias_flow,
        state_storage: &native_plan.state_storage,
        state_values: &native_plan.state_values,
        data: &native_plan.data,
        instructions: &native_plan.instructions,
        control_flow: &native_plan.control_flow,
        runtime_flow: &native_plan.runtime_flow,
        state_dispatch: &native_plan.state_dispatch,
        state_guards: &native_plan.state_guards,
        runtime_bodies: &native_plan.runtime_bodies,
        runtime_branching_calls: &native_plan.runtime_branching_calls,
        runtime_dispatch_loop: &native_plan.runtime_dispatch_loop,
        runtime_storage: &native_plan.runtime_storage,
        runtime_text: &native_plan.runtime_text,
        layouts: &native_plan.layouts,
        machine_code: &native_plan.machine_code,
        encoded_machine: &native_plan.encoded_machine,
        object: &native_plan.object,
        relocations: &native_plan.relocations,
    };
    let output = omega_backend_report::native_report_text(native_surface, &report_input);
    artifacts.write_text("09_native_plan.txt", &output)
}
