use super::*;
use crate::flow::mutation::receiver::{
    call_receiver_mutated_place, canonical_receiver_place_for_call_site,
};

#[derive(Debug, Clone, Default)]
pub(crate) struct StateMutationSummaryCache {
    pub(crate) states: Vec<StateMutationSummary>,
}

#[derive(Debug, Clone)]
pub(crate) struct StateMutationSummary {
    pub(crate) state_symbol: SymbolHandle,
    pub(crate) writes: Vec<CanonicalPlace>,
}

pub(crate) fn instantiate_known_call_mutation_summary_places(
    program: &psi_typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow_call: &BorrowCallFact,
    cache: &mut StateMutationSummaryCache,
) -> Option<Vec<CanonicalPlace>> {
    let target_state = find_state(program, borrow_call.target_symbol)?;
    let summary_places = state_mutation_summary_places(program, cache, target_state);
    if summary_places.is_empty() {
        if core_method_mutates_receiver(program, target_state) {
            return call_receiver_mutated_place(
                program,
                caller_machine_symbol,
                caller_state_symbol,
                borrow_call,
            )
            .map(|place| vec![place])
            .or_else(|| Some(Vec::new()));
        }
        return Some(Vec::new());
    }

    let mut instantiated = Vec::new();
    for summary_place in summary_places {
        if let Some(place) = instantiate_call_relative_place(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow_call,
            summary_place,
        ) && !instantiated.contains(&place)
        {
            instantiated.push(place);
        }
    }

    Some(instantiated)
}

fn core_method_mutates_receiver(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
) -> bool {
    let Some(machine) = program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|candidate| candidate.symbol == state.symbol)
    }) else {
        return false;
    };

    let method_name = machine
        .name
        .as_str()
        .rsplit_once("::")
        .map(|(_, method)| method)
        .unwrap_or_else(|| state.name.as_str());
    if !matches!(method_name, "as_mut_slice" | "index_mut" | "pop" | "push") {
        return false;
    }

    let is_vec_attached_machine = machine
        .attached_data
        .as_ref()
        .is_some_and(|attached_data| attached_data.as_str() == "Vec");
    let is_vec_method_machine = machine.name.as_str().starts_with("Vec::");

    is_vec_attached_machine || is_vec_method_machine
}

fn state_mutation_summary_places<'cache>(
    program: &psi_typed_trees::TypedTrees,
    cache: &'cache mut StateMutationSummaryCache,
    state: &psi_typed_trees::state::State,
) -> &'cache [CanonicalPlace] {
    if !cache
        .states
        .iter()
        .any(|entry| entry.state_symbol == state.symbol)
    {
        let writes = collect_state_mutation_summary_places(program, state);
        cache.states.push(StateMutationSummary {
            state_symbol: state.symbol,
            writes,
        });
    }

    cache
        .states
        .iter()
        .find(|entry| entry.state_symbol == state.symbol)
        .map(|entry| entry.writes.as_slice())
        .unwrap_or(&[])
}

fn collect_state_mutation_summary_places(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
) -> Vec<CanonicalPlace> {
    let machine_symbol = program
        .machines()
        .iter()
        .find_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .any(|candidate| candidate.symbol == state.symbol)
                .then_some(machine.symbol)
        })
        .unwrap_or_else(SymbolHandle::invalid);
    let parameter_symbols: Vec<_> = program
        .state_parameters(state)
        .iter()
        .map(|parameter| parameter.symbol)
        .collect();
    let mut writes = Vec::new();

    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        let StatementNode::Assignment(assignment) = statement else {
            continue;
        };
        let Some(mut place) = canonical_place_from_expression_in_state(
            program,
            state.symbol,
            statement_index,
            assignment.target,
        ) else {
            continue;
        };
        normalize_write_only_range_place(program, state.symbol, &mut place);
        let psi_facts::PlaceRoot::Symbol(root_symbol) = place.root else {
            continue;
        };
        if (parameter_symbols.contains(&root_symbol) || root_symbol == machine_symbol)
            && !writes.contains(&place)
        {
            writes.push(place);
        }
    }

    writes
}

fn instantiate_call_relative_place(
    program: &psi_typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow_call: &BorrowCallFact,
    relative_place: &CanonicalPlace,
) -> Option<CanonicalPlace> {
    let psi_facts::PlaceRoot::Symbol(parameter_symbol) = relative_place.root else {
        return None;
    };
    let call_site = find_call_site(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    )?;
    let target_state = find_state(program, borrow_call.target_symbol)?;
    let target_machine_symbol = program
        .machines()
        .iter()
        .find_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .any(|candidate| candidate.symbol == target_state.symbol)
                .then_some(machine.symbol)
        })
        .unwrap_or_else(SymbolHandle::invalid);
    let mut argument_index = 0usize;

    for parameter in program.state_parameters(target_state) {
        let base_place = if parameter.is_self {
            if parameter.symbol != parameter_symbol && target_machine_symbol != parameter_symbol {
                continue;
            }
            canonical_receiver_place_for_call_site(
                program,
                caller_machine_symbol,
                caller_state_symbol,
                &call_site,
            )
        } else {
            let argument = call_site_argument_expressions(program, &call_site)
                .get(argument_index)
                .copied();
            argument_index = argument_index.saturating_add(1);
            if parameter.symbol != parameter_symbol {
                continue;
            }
            argument.and_then(|expression| {
                canonical_place_from_expression_in_state(
                    program,
                    caller_state_symbol,
                    borrow_call.statement_index,
                    expression,
                )
                .or_else(|| canonical_place_from_expression(program, expression))
            })
        }?;

        let mut instantiated = base_place;
        instantiated
            .segments
            .extend(relative_place.segments.iter().copied());
        return Some(instantiated);
    }

    None
}
