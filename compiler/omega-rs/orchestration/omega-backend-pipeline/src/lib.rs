use omega_core::parallel::WorkerPoolHandle;
use omega_target::NativeTarget;
use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use std::sync::Arc;

use omega_control_flow::ControlFlowPlan;

mod builder;
mod entry;
mod skeleton;
mod timing;

pub use builder::render_frame_slot_table;
pub use omega_backend_plan::{BackendPlan, BackendPlanPhaseTiming};

/// `freestanding`: the selected build trusts no ambient host packages, so use
/// an empty host ABI baseline (no implicit bindings/lowerings or import
/// thunks). This policy is independent of image subsystem metadata.
pub fn build_backend_plan_from_control_flow_with_workers(
    program: Arc<CheckedTrees>,
    selected_provider_plans: Arc<omega_effects::SelectedProviderPlanFacts>,
    entry_machine_name: Option<&str>,
    target: NativeTarget,
    freestanding: bool,
    external_binding_rows: &[omega_calling_conventions::ExternalBindingRow],
    control_flow: Arc<ControlFlowPlan>,
    workers: WorkerPoolHandle,
) -> Result<BackendPlan, Diagnostic> {
    builder::build_backend_plan_from_control_flow_with_workers(
        program,
        selected_provider_plans,
        entry_machine_name,
        target,
        freestanding,
        external_binding_rows,
        control_flow,
        workers,
    )
}
