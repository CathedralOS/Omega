//! Ordered scalar exits retain every authored guard and selected destination.
//! Coverage is separate from evaluation: a final case guard is not a wildcard.

use super::*;
use checked_trees::{CheckedScalarGuardedExit, CheckedScalarGuardedTail};

pub(super) fn build(
    program: &TypedTrees,
    expressions: &checked_trees::CheckedScalarExpressionPlans,
) -> (
    arena::Arena<CheckedScalarGuardedExit>,
    Vec<CheckedScalarGuardedTail>,
) {
    let mut exits = arena::Arena::default();
    let mut tails = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            if let Some((arms, fallback)) = tail(program, machine, state, expressions) {
                let mut retained = arena::HandleSpan::empty();
                for arm in arms {
                    exits.append_to_span(&mut retained, arm);
                }
                tails.push(CheckedScalarGuardedTail {
                    state: state.symbol,
                    arms: retained,
                    fallback,
                });
            }
        }
    }
    (exits, tails)
}

fn tail(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expressions: &checked_trees::CheckedScalarExpressionPlans,
) -> Option<(
    Vec<CheckedScalarGuardedExit>,
    Option<CheckedScalarBranchDestination>,
)> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let prefix = statements
        .iter()
        .take_while(|statement| {
            matches!(
                statement,
                StatementNode::LocalData(_) | StatementNode::Assignment(_) | StatementNode::Call(_)
            )
        })
        .count();
    let mut arms = Vec::new();
    let mut fallback = None;
    let mut case_subject = None;
    let mut cases = Vec::new();
    let mut exact_cases = true;
    for (index, statement) in statements.iter().enumerate().skip(prefix) {
        let ordinal = u32::try_from(index).ok()?;
        if fallback.is_some() {
            return None;
        }
        let StatementNode::Transition(transition) = statement else {
            if matches!(statement, StatementNode::Expression(_)) && !arms.is_empty() {
                fallback = Some(CheckedScalarBranchDestination::Return {
                    statement_ordinal: ordinal,
                    is_continuation: false,
                });
                continue;
            }
            return None;
        };
        if transition.continuation.is_valid() {
            return None;
        }
        let destination = checked_branch_destination(program, machine, ordinal, transition, false)?;
        match transition.guard {
            TransitionGuardNode::Always if !arms.is_empty() => fallback = Some(destination),
            TransitionGuardNode::When(guard) => {
                let mut selected = expressions.expressions.iter().filter(|expression| {
                    expression.state == state.symbol
                        && expression.statement_ordinal == ordinal
                        && expression.role == checked_trees::CheckedScalarExpressionRole::Guard
                });
                let expression = selected.next()?;
                if selected.next().is_some() {
                    return None;
                }
                let checked_trees::CheckedScalarExpression::Boolean(boolean) =
                    &expression.expression
                else {
                    return None;
                };
                if let checked_trees::CheckedBooleanExpression::StructuralCaseMembership {
                    subject,
                    ..
                } = boolean.as_ref()
                {
                    let (_, case) = crate::proof::exact_outcome_case_test(program, guard)?;
                    if case_subject.as_ref().is_some_and(|prior| prior != subject) {
                        exact_cases = false;
                    }
                    case_subject = Some(subject.clone());
                    if cases.contains(&case) {
                        exact_cases = false;
                    }
                    cases.push(case);
                } else {
                    exact_cases = false;
                }
                arms.push(CheckedScalarGuardedExit {
                    guard_statement_ordinal: ordinal,
                    destination,
                });
            }
            _ => return None,
        }
    }
    // Existing binary control retains its established representation. Larger
    // ladders use this shared roster rather than a source-family body planner.
    if arms.len() < 2
        || (arms.len() == 2
            && fallback.is_none()
            && guards::complementary(expressions, state.symbol, u32::try_from(prefix).ok()?))
    {
        return None;
    }
    if fallback.is_none() {
        if !exact_cases {
            return None;
        }
        let first = *cases.first()?;
        let owner = program.symbols.get(first).parent;
        let data = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == owner)?;
        let mut expected = Vec::new();
        for member in program.data_members(data) {
            let typed_trees::data::DataMember::Variant(variant) = member else {
                return None;
            };
            expected.push(variant.symbol);
        }
        if expected.len() != cases.len() || expected.iter().any(|case| !cases.contains(case)) {
            return None;
        }
    }
    Some((arms, fallback))
}
