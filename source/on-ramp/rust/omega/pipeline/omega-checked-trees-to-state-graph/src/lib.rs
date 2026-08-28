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
    reject_unsupported_atomic_operations(&program)?;
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

fn reject_unsupported_atomic_operations(program: &CheckedTrees) -> Result<(), Diagnostic> {
    if program
        .expression_table
        .iter_expressions()
        .any(|(_, expression)| {
            matches!(
                expression,
                psi_checked_trees::expression::ExpressionNode::Atomic(atomic)
                    if matches!(
                        atomic.ordering,
                        psi_language_core::atomic::AtomicOrderingPlan::CompareExchangeOnce { .. }
                    )
            )
        })
    {
        return Err(Diagnostic::error(
            "observing single-attempt `compare_exchange_once` is represented in checked Psi, but the legacy native state-graph pipeline has no result carrier or target operation identity and refuses to lower it",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::build_state_graph;
    use psi_checked_trees::CheckedTrees;
    use psi_checked_trees::expression::{
        ExpressionNode, TableAtomicExpression, TableBinaryExpression,
    };
    use psi_language_core::atomic::{AtomicOrderingPlan, MemoryOrdering};
    use psi_numerics::literals::IntegerLiteral;

    #[test]
    fn rejects_forged_single_attempt_compare_exchange_anywhere_in_checked_expressions() {
        let mut checked = CheckedTrees::default();
        let value = checked
            .typed
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
        let observed = checked
            .typed
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
        let once = checked
            .typed
            .expression_table
            .insert(ExpressionNode::Atomic(TableAtomicExpression {
                value,
                result: observed,
                ordering: AtomicOrderingPlan::CompareExchangeOnce {
                    success: MemoryOrdering::ReceivePublish,
                    failure: MemoryOrdering::Receive,
                },
            }));
        // Keep the forged operation nested in a detached checked subtree. The
        // lowering fence must inspect the complete checked expression arena,
        // not only the roots currently reached by ordinary source lowering.
        checked
            .typed
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: value,
                operator: psi_checked_trees::expression::BinaryOperator::Add,
                right: once,
            }));
        let diagnostic = build_state_graph(&checked)
            .expect_err("single-attempt compare-exchange has no Omega lowering");

        assert!(diagnostic.message.contains("compare_exchange_once"));
        assert!(diagnostic.message.contains("no result carrier"));
        assert!(diagnostic.message.contains("refuses to lower"));
    }

    #[test]
    fn decisive_compare_exchange_remains_outside_the_single_attempt_fence() {
        let mut checked = CheckedTrees::default();
        let value = checked
            .typed
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
        checked
            .typed
            .expression_table
            .insert(ExpressionNode::Atomic(TableAtomicExpression {
                value,
                result: value,
                ordering: AtomicOrderingPlan::CompareExchange {
                    success: MemoryOrdering::ReceivePublish,
                    failure: MemoryOrdering::Receive,
                },
            }));
        build_state_graph(&checked)
            .expect("existing decisive compare-exchange must not trip the Once fence");
    }
}
