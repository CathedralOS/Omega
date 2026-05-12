use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::WorkerPoolHandle;
use omega_target::NativeTarget;
use omega_checked_trees::Program;
use std::sync::Arc;

use omega_control_flow::ControlFlowPlan;

mod builder;
mod entry;
mod skeleton;
mod timing;

pub use omega_backend_plan::{BackendPlan, BackendPlanPhaseTiming};

pub fn build_backend_plan_from_control_flow_with_workers(
    program: Arc<Program>,
    target: NativeTarget,
    control_flow: Arc<ControlFlowPlan>,
    workers: WorkerPoolHandle,
) -> Result<BackendPlan, Diagnostic> {
    builder::build_backend_plan_from_control_flow_with_workers(
        program,
        target,
        control_flow,
        workers,
    )
}
