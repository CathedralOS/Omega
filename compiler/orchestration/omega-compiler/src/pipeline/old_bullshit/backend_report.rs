use super::artifacts::ArtifactWriter;

use omega_artifacts::BackendSurfaceReport;
use omega_backend_plan::BackendPlan;
use omega_backend_report::{BackendReportInput, BackendReportPhaseTiming};
use omega_core::diagnostics::Diagnostic;

pub(crate) fn write_backend_report(
    artifacts: &ArtifactWriter,
    backend_surface: &BackendSurfaceReport,
    backend_plan: &BackendPlan,
) -> Result<(), Diagnostic> {
    let phase_timings = backend_plan
        .phase_timings
        .iter()
        .map(|timing| BackendReportPhaseTiming {
            phase: timing.phase.clone(),
            microseconds: timing.microseconds,
            allocations: timing.allocations,
        })
        .collect::<Vec<_>>();
    let report_input = BackendReportInput {
        target: backend_plan.target,
        entry_key: backend_plan.entry_key,
        phase_timings: &phase_timings,
        host_abi: &backend_plan.host_abi,
        host_calls: &backend_plan.host_calls,
        state_calls: &backend_plan.state_calls,
        alias_flow: &backend_plan.alias_flow,
        state_storage: &backend_plan.state_storage,
        state_values: &backend_plan.state_values,
        data: &backend_plan.data,
        instructions: &backend_plan.instructions,
        control_flow: &backend_plan.control_flow,
        runtime_flow: &backend_plan.runtime_flow,
        state_dispatch: &backend_plan.state_dispatch,
        state_guards: &backend_plan.state_guards,
        runtime_bodies: &backend_plan.runtime_bodies,
        runtime_branching_calls: &backend_plan.runtime_branching_calls,
        runtime_dispatch_loop: &backend_plan.runtime_dispatch_loop,
        runtime_storage: &backend_plan.runtime_storage,
        runtime_text: &backend_plan.runtime_text,
        layouts: &backend_plan.layouts,
        machine_code: &backend_plan.machine_code,
        encoded_machine: &backend_plan.encoded_machine,
        object: &backend_plan.object,
        relocations: &backend_plan.relocations,
    };
    let output = omega_backend_report::backend_report_text(backend_surface, &report_input);
    artifacts.write_text("09_backend_plan.txt", &output)
}
