//! The shared subject of a runtime-indexed case dispatch.
//!
//! Symbol resolution hoists `transition self.items[self.w] { Color::Red -> ..
//! }` into one subject, `let __hoist_N = &self.items[self.w];`, so every arm
//! tests the same local and exhaustiveness still groups the arms
//! (`match_subject_hoisting.rs` in stage 02). The shared borrow names the
//! element each arm observes; it is never read as a value, moved, or passed
//! on. A Unit plan therefore erases it: the `let` plans no operation, and each
//! arm's membership observes the borrowed place itself, a
//! `StructuralCaseMembership` whose path carries the runtime element
//! (`items`, `RuntimeIndex`). That observation reads "some element", keys no
//! case fact, and owns the element's bound, which Terminal re-proves against
//! the array's extent at every arm.
//!
//! Erasure is sound only while the subject is exactly that: an immutable
//! shared borrow of a runtime-selected place, followed in its state by nothing
//! but the dispatch's transitions, each of which tests the local's case. No
//! statement between the borrow and an arm can write the array or the
//! selector, so re-reading the selector at each arm observes the element the
//! borrow named. Anything else keeps the ordinary local planning, which
//! declines a reference local it cannot establish.
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::{StatementNode, TableLocalData, TransitionGuardNode};

/// The borrowed place `local` erases to, when it is a runtime-indexed case
/// dispatch subject: its initializer's borrow target.
pub(crate) fn erased_subject(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    local_statement: usize,
    local: &TableLocalData,
) -> Option<ExpressionHandle> {
    if local.is_mutable || !local.symbol.is_valid() {
        return None;
    }
    let ExpressionNode::Borrow(borrow) = program.expression_table.expression(local.initial_value)
    else {
        return None;
    };
    if borrow.access != language_core::ReferenceAccess::Shared {
        return None;
    }
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        local_statement,
        borrow.target,
    )?;
    if !matches!(place.root, facts::PlaceRoot::Symbol(_))
        || !place
            .segments
            .iter()
            .any(|segment| matches!(segment, facts::PlaceSegment::Index { .. }))
    {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let arms = statements.get(local_statement + 1..)?;
    if arms.is_empty() {
        return None;
    }
    for statement in arms {
        let StatementNode::Transition(transition) = statement else {
            return None;
        };
        match transition.guard {
            TransitionGuardNode::Always => {}
            TransitionGuardNode::When(guard) => {
                let (subject, _) = crate::proof::exact_outcome_case_test(program, guard)?;
                let ExpressionNode::Name(name) = program.expression_table.expression(subject)
                else {
                    return None;
                };
                if name.symbol != local.symbol {
                    return None;
                }
            }
        }
    }
    Some(borrow.target)
}

/// The borrowed place a membership subject names through an erased dispatch
/// subject, with the statement index of the erased `let`.
pub(crate) fn observed_place(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    subject: ExpressionHandle,
) -> Option<(ExpressionHandle, usize)> {
    let ExpressionNode::Name(name) = program.expression_table.expression(subject) else {
        return None;
    };
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .find_map(|(index, statement)| match statement {
            StatementNode::LocalData(local) if local.symbol == name.symbol => {
                erased_subject(program, state, index, local).map(|place| (place, index))
            }
            _ => None,
        })
}
