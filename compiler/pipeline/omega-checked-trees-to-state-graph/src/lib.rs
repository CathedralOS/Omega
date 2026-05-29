use omega_checked_trees::CheckedTrees;
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_state_graph::StateGraph;
use std::sync::Arc;

mod borrows;
mod boundaries;
mod builder;
mod capacity;
mod contracts;
mod facts;
mod machine_metadata;
mod merge;
mod ownership;
mod remap;
mod segments;
mod states;
mod transitions;
mod values;

pub fn build_state_graph(program: &CheckedTrees) -> Result<StateGraph, Diagnostic> {
    let workers = WorkerPool::with_available_parallelism();

    build_state_graph_with_workers(Arc::new(program.clone()), workers.handle())
}

pub fn build_state_graph_owned(program: CheckedTrees) -> Result<StateGraph, Diagnostic> {
    let workers = WorkerPool::with_available_parallelism();

    build_state_graph_with_workers(Arc::new(program), workers.handle())
}

pub fn build_state_graph_with_workers(
    program: Arc<CheckedTrees>,
    workers: WorkerPoolHandle,
) -> Result<StateGraph, Diagnostic> {
    builder::build_state_graph_with_workers(program, workers)
}
