use crate::target::NativeTarget;
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_typed_program::Program;
use std::sync::Arc;

mod builder;
mod model;
mod timing;

pub use model::{NativePlan, NativePlanPhaseTiming};

pub fn build_native_plan(
    program: &Program,
    target: NativeTarget,
) -> Result<NativePlan, Diagnostic> {
    let workers = WorkerPool::with_available_parallelism();

    build_native_plan_with_workers(Arc::new(program.clone()), target, workers.handle())
}

pub fn build_native_plan_with_workers(
    program: Arc<Program>,
    target: NativeTarget,
    workers: WorkerPoolHandle,
) -> Result<NativePlan, Diagnostic> {
    builder::build_native_plan_with_workers(program, target, workers)
}
