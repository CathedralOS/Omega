use super::*;
use crate::lookup::expression_root_symbol;

#[derive(Debug, Clone, Default)]
pub(crate) struct StateMutationSummaryCache {
    pub(crate) states: Vec<StateMutationSummary>,
}

#[derive(Debug, Clone)]
pub(crate) struct StateMutationSummary {
    pub(crate) state_symbol: SymbolHandle,
    pub(crate) writes: Vec<CanonicalPlace>,
}

pub(crate) fn call_mutated_places(
    program: &omega_typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    state_mutation_summaries: &mut StateMutationSummaryCache,
) -> Vec<CanonicalPlace> {
    let summarized_places = instantiate_call_mutation_summary_places(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow_call,
        state_mutation_summaries,
    );
    if !summarized_places.is_empty() {
        return summarized_places;
    }

    let mut places = Vec::new();
    for access in borrow.argument_accesses.span_or_empty(borrow_call.accesses) {
        if access.kind == BorrowAccessKind::Mutable
            && let Some(place) = canonical_place_from_symbol(access.root_symbol)
            && !places.contains(&place)
        {
            places.push(place);
        }
    }

    if let Some(call_site) = find_call_site(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    ) && let Some(target_state) = find_state(program, borrow_call.target_symbol)
    {
        let mut argument_index = 0usize;
        for parameter in program.state_parameters(target_state) {
            if parameter.is_self {
                continue;
            }

            let argument = call_site_argument_expressions(program, &call_site)
                .get(argument_index)
                .copied();
            argument_index = argument_index.saturating_add(1);

            if !parameter.is_mutable {
                continue;
            }

            if let Some(argument) = argument
                && let Some(place) = canonical_place_from_expression(program, argument)
                && !places.contains(&place)
            {
                places.push(place);
            }
        }
    }

    if borrow_call.has_receiver
        && call_receiver_is_mutable(program, borrow, borrow_call)
        && let Some(place) = call_receiver_mutated_place(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow_call,
        )
        && !places.contains(&place)
    {
        places.push(place);
    }

    places
}

pub(crate) fn call_receiver_is_mutable(
    program: &omega_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
) -> bool {
    let Some((target_machine_symbol, target_state_symbol)) =
        contract_target_from_state_symbol(program, borrow_call.target_symbol)
    else {
        return false;
    };
    let Some(state) = find_state_in_machine(program, target_machine_symbol, target_state_symbol)
    else {
        return false;
    };
    program
        .state_parameters(state)
        .iter()
        .any(|parameter| parameter.is_self && parameter.is_mutable)
        || borrow_call.accesses.is_empty()
            && borrow.states.iter().any(|(_, flow_state)| {
                flow_state.machine_symbol == target_machine_symbol
                    && flow_state.state_symbol == target_state_symbol
                    && flow_state.mutable_parameter_count > 0
            })
}

pub(crate) fn call_may_mutate_contract_state(
    program: &omega_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
) -> bool {
    let Some((target_machine_symbol, target_state_symbol)) =
        contract_target_from_state_symbol(program, borrow_call.target_symbol)
    else {
        return false;
    };
    let Some(state) = find_state_in_machine(program, target_machine_symbol, target_state_symbol)
    else {
        return false;
    };
    let signature_mutability = program
        .state_parameters(state)
        .iter()
        .any(|parameter| parameter.is_mutable);
    let borrow_mutability = borrow.states.iter().any(|(_, flow_state)| {
        flow_state.machine_symbol == target_machine_symbol
            && flow_state.state_symbol == target_state_symbol
            && flow_state.mutable_parameter_count > 0
    });

    signature_mutability
        || borrow_mutability
        || call_receiver_is_mutable(program, borrow, borrow_call)
}

pub(crate) fn call_receiver_mutated_place(
    program: &omega_typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow_call: &BorrowCallFact,
) -> Option<CanonicalPlace> {
    let call_site = find_call_site(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    )?;
    match call_site {
        CallSite::Statement(statement) => {
            if let Some(path) = statement_call_receiver_path(program, statement) {
                return Some(CanonicalPlace {
                    root: omega_facts::PlaceRoot::Symbol(path.head_symbol()),
                    segments: path
                        .member_symbols()
                        .iter()
                        .skip(1)
                        .copied()
                        .map(|symbol| omega_facts::PlaceSegment::Field { symbol })
                        .collect(),
                });
            }
            canonical_place_from_symbol(statement.receiver_symbol)
        }
        CallSite::Expression(call) => {
            if call.receiver.is_valid() {
                canonical_place_from_expression(program, call.receiver)
            } else {
                let caller_state =
                    find_state_in_machine(program, caller_machine_symbol, caller_state_symbol)?;
                let self_parameter = program
                    .state_parameters(caller_state)
                    .iter()
                    .find(|parameter| parameter.is_self)?;
                canonical_place_from_symbol(self_parameter.symbol)
            }
        }
    }
}

fn instantiate_call_mutation_summary_places(
    program: &omega_typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow_call: &BorrowCallFact,
    cache: &mut StateMutationSummaryCache,
) -> Vec<CanonicalPlace> {
    let Some(target_state) = find_state(program, borrow_call.target_symbol) else {
        return Vec::new();
    };
    let summary_places = state_mutation_summary_places(program, cache, target_state);
    if summary_places.is_empty() {
        return Vec::new();
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

    instantiated
}

fn state_mutation_summary_places<'cache>(
    program: &omega_typed_trees::TypedTrees,
    cache: &'cache mut StateMutationSummaryCache,
    state: &omega_typed_trees::state::State,
) -> &'cache [CanonicalPlace] {
    if !cache.states.iter().any(|entry| entry.state_symbol == state.symbol) {
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
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
) -> Vec<CanonicalPlace> {
    let parameter_symbols: Vec<_> = program
        .state_parameters(state)
        .iter()
        .map(|parameter| parameter.symbol)
        .collect();
    let mut writes = Vec::new();

    for statement in program.statement_table.statements(state.statement_nodes) {
        let StatementNode::Assignment(assignment) = statement else {
            continue;
        };
        let Some(place) = canonical_place_from_expression(program, assignment.target) else {
            continue;
        };
        let omega_facts::PlaceRoot::Symbol(root_symbol) = place.root else {
            continue;
        };
        if parameter_symbols.contains(&root_symbol) && !writes.contains(&place) {
            writes.push(place);
        }
    }

    writes
}

fn instantiate_call_relative_place(
    program: &omega_typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow_call: &BorrowCallFact,
    relative_place: &CanonicalPlace,
) -> Option<CanonicalPlace> {
    let omega_facts::PlaceRoot::Symbol(parameter_symbol) = relative_place.root else {
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
    let mut argument_index = 0usize;

    for parameter in program.state_parameters(target_state) {
        let base_place = if parameter.is_self {
            if parameter.symbol != parameter_symbol {
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
            argument.and_then(|expression| canonical_place_from_expression(program, expression))
        }?;

        let mut instantiated = base_place;
        instantiated
            .segments
            .extend(relative_place.segments.iter().copied());
        return Some(instantiated);
    }

    None
}

fn canonical_receiver_place_for_call_site(
    program: &omega_typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    call_site: &CallSite<'_>,
) -> Option<CanonicalPlace> {
    match call_site {
        CallSite::Statement(statement) => {
            if let Some(path) = statement_call_receiver_path(program, statement) {
                return Some(CanonicalPlace {
                    root: omega_facts::PlaceRoot::Symbol(path.head_symbol()),
                    segments: path
                        .member_symbols()
                        .iter()
                        .skip(1)
                        .copied()
                        .map(|symbol| omega_facts::PlaceSegment::Field { symbol })
                        .collect(),
                });
            }
            canonical_place_from_symbol(statement.receiver_symbol)
        }
        CallSite::Expression(call) => {
            if call.receiver.is_valid() {
                return canonical_place_from_expression(program, call.receiver);
            }

            let caller_state =
                find_state_in_machine(program, caller_machine_symbol, caller_state_symbol)?;
            let self_parameter = program
                .state_parameters(caller_state)
                .iter()
                .find(|parameter| parameter.is_self)?;
            canonical_place_from_symbol(self_parameter.symbol)
        }
    }
}

pub(crate) fn statement_mutated_place(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    statement: &StatementNode,
) -> Option<CanonicalPlace> {
    match statement {
        StatementNode::Assignment(assignment) => {
            canonical_place_from_expression(program, assignment.target).or_else(|| {
                expression_root_symbol(assignment.target, &program.expression_table, machine.symbol)
                    .and_then(canonical_place_from_symbol)
            })
        }
        _ => None,
    }
}
