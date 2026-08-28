use omega_state_graph::StateGraph;
use psi_arena::{Arena, HandleSpan};
use psi_checked_trees::CheckedTrees;
use psi_checked_trees::expression::ExpressionHandle;
use psi_checked_trees::statement::{TableTransition, TransitionGuardNode};

use super::StateSegment;
use crate::runtime_expressions::copy_runtime_expression_slice;

#[derive(Debug, Clone)]
pub(crate) enum SegmentTransition {
    Tree {
        statement_index: usize,
        table: TableTransition,
    },
    ReturnExpression {
        statement_index: usize,
        expression: ExpressionHandle,
    },
    BranchCall {
        statement_index: usize,
        has_continuation_segment: bool,
    },
}

impl Default for SegmentTransition {
    fn default() -> Self {
        Self::Tree {
            statement_index: 0,
            table: TableTransition::default(),
        }
    }
}

pub(crate) fn branch_call_transitions(
    statement_index: usize,
    has_continuation_segment: bool,
    segment_transitions: &mut Arena<SegmentTransition>,
) -> HandleSpan<SegmentTransition> {
    let mut transitions = HandleSpan::empty();
    segment_transitions.append_to_span(
        &mut transitions,
        SegmentTransition::BranchCall {
            statement_index,
            has_continuation_segment,
        },
    );
    transitions
}

pub(crate) fn segment_has_unconditional_transition(
    segment: &StateSegment,
    segment_transitions: &Arena<SegmentTransition>,
) -> bool {
    segment_transitions
        .span_or_empty(segment.transitions)
        .iter()
        .any(|transition| match transition {
            SegmentTransition::Tree { table, .. } => {
                matches!(table.guard, TransitionGuardNode::Always)
            }
            SegmentTransition::ReturnExpression { .. } => true,
            SegmentTransition::BranchCall { .. } => true,
        })
}

pub(crate) fn copy_statement_expression_span(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    expressions: HandleSpan<ExpressionHandle>,
) -> HandleSpan<ExpressionHandle> {
    copy_runtime_expression_slice(
        state_graph,
        program,
        program.statement_table.expression_handles(expressions),
    )
}

pub(crate) fn table_transition_guard_expression(transition: TableTransition) -> ExpressionHandle {
    match transition.guard {
        TransitionGuardNode::Always => ExpressionHandle::invalid(),
        TransitionGuardNode::When(expression) => expression,
    }
}
