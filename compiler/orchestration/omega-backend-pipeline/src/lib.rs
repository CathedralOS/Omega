use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::WorkerPoolHandle;
use omega_target::NativeTarget;
use omega_typed_program::Program;
use std::sync::Arc;

use omega_control_flow::ControlFlowPlan;

mod builder;
mod entry;
mod skeleton;
mod timing;

pub use omega_backend_plan::{NativePlan, NativePlanPhaseTiming};

pub fn build_native_plan_from_control_flow_with_workers(
    program: Arc<Program>,
    target: NativeTarget,
    control_flow: Arc<ControlFlowPlan>,
    workers: WorkerPoolHandle,
) -> Result<NativePlan, Diagnostic> {
    builder::build_native_plan_from_control_flow_with_workers(
        program,
        target,
        control_flow,
        workers,
    )
}
