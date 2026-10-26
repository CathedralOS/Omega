//! Project checked predicate consequences onto the exact application actuals.
//!
//! The collection-predicate explorer owns body interpretation, selected builtin
//! comparisons and bounded all-path exploration. This adapter consumes its
//! true-return hulls, never the callee's preconditions or a desired bound.
//! Runtime slices contribute only descriptor extent knowledge; their unknown
//! elements cannot be read by that abstraction. Existing value identities,
//! mutation invalidation and all-arrival joins own the projected facts' lifetime.

use super::super::ordered_values::{Operand, Relation, RelationConstraint};
use super::{ExpressionHandle, Interval, Machine, State, ValueEnvironment};
use crate::proof_contracts::collection_predicates::{self, ArgumentKnowledge};
use numerics::literals::IntegerLiteral;

pub(super) fn narrow(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &mut ValueEnvironment,
    condition: ExpressionHandle,
) -> bool {
    let Some(state) = state else {
        return false;
    };
    let Some(application) =
        collection_predicates::predicate_application(program, machine.symbol, condition)
    else {
        return false;
    };
    // A later actual may write storage an earlier actual already copied.
    // Until this reader receives evaluated actual snapshots, only complete
    // empty operand effects can project observations back to caller places.
    if application
        .arguments
        .iter()
        .any(|argument| crate::machine_calls::calls::expression_has_calls(program, *argument))
        && !crate::CallFrameResolver::new(program).is_some_and(|frames| {
            frames
                .expression_may_write_paths(machine, condition)
                .is_some_and(|paths| paths.is_empty())
        })
    {
        return false;
    }
    let Some(callee) = program.machines().get(application.machine) else {
        return false;
    };
    let Some(entry) = program.machine_states(callee).first() else {
        return false;
    };
    let parameters = program.state_parameters(entry);
    if parameters.len() != application.arguments.len() {
        return false;
    }
    // Start from complete declared carriers. Contextual facts are not needed
    // to discover consequences, and no convenient caller bound is assumed.
    let Some(knowledge) = parameters
        .iter()
        .map(|parameter| {
            collection_predicates::unknown_knowledge(program, parameter.type_reference)
        })
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    let Some(unfolding) = collection_predicates::unfold(program, application.machine, &knowledge)
    else {
        return false;
    };
    for (position, (argument, knowledge)) in
        application.arguments.iter().zip(&knowledge).enumerate()
    {
        let extent = match knowledge {
            ArgumentKnowledge::Integer(_) => false,
            ArgumentKnowledge::CollectionExtent(_) => true,
            _ => continue,
        };
        let Some(consequence) = unfolding.when_true(position) else {
            continue;
        };
        let [values] = consequence.as_slice() else {
            continue;
        };
        let Some((low, high)) = values.bounds() else {
            continue;
        };
        let subject =
            super::super::ordered_values::operand(program, machine, state, *argument, None);
        let path = super::super::place_path(program, *argument)
            .or_else(|| subject.as_ref().and_then(Operand::static_path))
            .map(|path| if extent { format!("{path}.len") } else { path });
        let interval = Interval {
            low: i64::try_from(low).ok(),
            high: i64::try_from(high).ok(),
        };
        if let Some(path) = path {
            environment.narrow(path, interval);
        }
        // A captured structural projection can own a precise interval even
        // when the optional ordered-expression algebra has no term for it.
        // Keep the enclosure; relation publication has its own admission.
        let Some(mut subject) = subject else { continue };
        if extent {
            subject = Operand::CollectionLength(Box::new(subject));
        }
        for (bound, lower) in [(interval.low, true), (interval.high, false)] {
            let Some(bound) = bound else {
                continue;
            };
            let bound = Operand::Integer(IntegerLiteral::from_value(bound), None);
            let relation = Relation {
                left: if lower {
                    subject.clone()
                } else {
                    bound.clone()
                },
                right: if lower { bound } else { subject.clone() },
                constraint: RelationConstraint::MinimumDifference(0),
            };
            if !environment.ordered_values.contains(&relation) {
                environment.ordered_values.push(relation);
            }
        }
    }
    true
}
