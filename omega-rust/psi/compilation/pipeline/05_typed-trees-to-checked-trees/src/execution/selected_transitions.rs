//! Executable selection over canonical transition coordinates. Source formation
//! retains every arm; a closed static dispatch plans only its admitted outcome.
use typed_trees::statement::{StatementNode, TableTransition, TransitionGuardNode};

pub(super) struct SelectedTransition<'source> {
    pub statement_ordinal: u32,
    pub transition: &'source TableTransition,
}

/// Resolve a complete static prefix, never speculate past an unknown guard.
/// Guards read here are closed static Boolean values, so omitting their runtime
/// evaluation removes no call, write, loan, or trapping operand. Earlier local
/// initializers still belong to the ordinary statement sequence.
pub(super) fn selected_transition<'source>(
    program: &dyn validation::CheckingInput,
    statements: &'source [StatementNode],
    start: usize,
) -> Option<SelectedTransition<'source>> {
    for (index, statement) in statements.iter().enumerate().skip(start) {
        let transition = statement.as_transition()?;
        // A two-destination transition has an independently owned continuation;
        // the ordinary control planner must retain that transaction as a whole.
        if transition.continuation.is_valid() {
            return None;
        }
        let selected = match transition.guard {
            TransitionGuardNode::Always => true,
            TransitionGuardNode::When(guard) => program.static_boolean_value(guard)?,
        };
        if !selected {
            continue;
        }
        if program.static_transition_type_context_is_unreachable(transition.target) {
            return None;
        }
        return Some(SelectedTransition {
            statement_ordinal: u32::try_from(index).ok()?,
            transition,
        });
    }
    None
}
