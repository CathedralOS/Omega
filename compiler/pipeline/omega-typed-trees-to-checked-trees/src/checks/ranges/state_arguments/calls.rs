use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::ExpressionHandle;
use omega_typed_trees::machine::Machine;

use super::{MergedFact, ParameterFacts, StateArgumentFacts};
use crate::checks::ranges::expressions::{expression_indexable_length, expression_integer_value};
use crate::checks::ranges::facts::RangeFacts;

pub(super) fn collect_state_argument_facts_for_call(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
    facts: &RangeFacts<'_>,
    target_symbol: SymbolHandle,
    target_name: Option<&omega_typed_trees::name::Identifier>,
    arguments: &[ExpressionHandle],
    collected: &mut Vec<StateArgumentFacts>,
) {
    let Some(target_state) = program.machine_states(machine).iter().find(|state| {
        (target_symbol.is_valid() && state.symbol == target_symbol)
            || target_name.is_some_and(|target_name| state.name == *target_name)
    }) else {
        return;
    };

    let entry = collected
        .iter_mut()
        .find(|entry| entry.state == target_state.symbol);
    let entry = if let Some(entry) = entry {
        entry
    } else {
        collected.push(StateArgumentFacts {
            state: target_state.symbol,
            parameters: program
                .state_parameters(target_state)
                .iter()
                .map(|parameter| ParameterFacts {
                    symbol: parameter.symbol,
                    name: parameter.name.to_string(),
                    is_self: parameter.is_self,
                    length: MergedFact::Unseen,
                    integer: MergedFact::Unseen,
                })
                .collect(),
        });
        collected
            .last_mut()
            .expect("state argument facts were just inserted")
    };

    for (parameter, argument) in entry
        .parameters
        .iter_mut()
        .filter(|parameter| !parameter.is_self)
        .zip(arguments.iter().copied())
    {
        parameter
            .length
            .merge(expression_indexable_length(program, facts, argument));
        parameter
            .integer
            .merge(expression_integer_value(program, facts, argument));
    }
}
