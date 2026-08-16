use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_state_graph::StateGraph;
use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use std::sync::Arc;

mod borrows;
mod boundaries;
mod builder;
mod capacity;
mod contracts;
mod dynamic_conformances;
mod facts;
mod machine_metadata;
mod merge;
mod ownership;
mod remap;
mod runtime_expressions;
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
    if program.machines().iter().any(|machine| {
        program.machine_states(machine).iter().any(|state| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|statement| {
                    matches!(
                        statement,
                        psi_checked_trees::statement::StatementNode::Transition(transition)
                            if matches!(
                                transition.exit,
                                psi_checked_trees::statement::TransitionExit::Crash(_)
                            )
                    )
                })
        })
    }) {
        return Err(Diagnostic::error(
            "explicit `crash Cause;` exits are represented in terminal Psi, but the legacy native state-graph pipeline has no target crash plan and refuses to lower them",
        ));
    }
    builder::build_state_graph_with_workers(program, workers)
}
