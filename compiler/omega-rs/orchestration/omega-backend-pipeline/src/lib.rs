use omega_checked_trees::CheckedTrees;
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::WorkerPoolHandle;
use omega_target::NativeTarget;
use std::sync::Arc;

use omega_control_flow::ControlFlowPlan;

mod builder;
mod entry;
mod skeleton;
mod timing;

pub use builder::render_frame_slot_table;
pub use omega_backend_plan::{BackendPlan, BackendPlanPhaseTiming};

/// `freestanding`: the selected target is no-host (`subsystem efi_application`)
/// -- build against an EMPTY host ABI plan (no bindings/lowerings, so no import
/// thunks; boundary calls fail with the ordinary missing-lowering diagnostic).
pub fn build_backend_plan_from_control_flow_with_workers(
    program: Arc<CheckedTrees>,
    target: NativeTarget,
    freestanding: bool,
    control_flow: Arc<ControlFlowPlan>,
    workers: WorkerPoolHandle,
) -> Result<BackendPlan, Diagnostic> {
    builder::build_backend_plan_from_control_flow_with_workers(
        program,
        target,
        freestanding,
        control_flow,
        workers,
    )
}
