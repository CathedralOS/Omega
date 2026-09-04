use super::*;
use crate::lookup::statement_call_receiver_members;

pub(crate) fn call_receiver_is_mutable(
    program: &psi_typed_trees::TypedTrees,
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
    program: &psi_typed_trees::TypedTrees,
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
    program: &psi_typed_trees::TypedTrees,
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
    canonical_receiver_place_for_call_site(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        &call_site,
    )
}

pub(crate) fn canonical_receiver_place_for_call_site(
    program: &psi_typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    call_site: &CallSite<'_>,
) -> Option<CanonicalPlace> {
    match call_site {
        CallSite::Statement(statement) => {
            if let Some(place) =
                canonical_self_receiver_path(program, caller_state_symbol, statement)
            {
                return Some(place);
            }

            canonical_place_from_symbol(statement.receiver_symbol)
        }
        CallSite::Expression { call, .. } => {
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
        CallSite::TransitionNamed { .. } => {
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

fn canonical_self_receiver_path(
    program: &psi_typed_trees::TypedTrees,
    caller_state_symbol: SymbolHandle,
    statement: &psi_typed_trees::statement::TableCall,
) -> Option<CanonicalPlace> {
    let members = statement_call_receiver_members(program, statement)?;
    if members
        .first()
        .is_none_or(|member| member.as_str() != "self")
    {
        return None;
    }

    let caller_state = find_state(program, caller_state_symbol)?;
    let self_parameter = program
        .state_parameters(caller_state)
        .iter()
        .find(|parameter| parameter.is_self)?;
    let mut place = CanonicalPlace {
        root: psi_facts::PlaceRoot::Symbol(self_parameter.symbol),
        segments: Vec::new(),
    };

    for member in members.iter().skip(1) {
        let symbol = resolve_self_receiver_member_symbol(program, &place, member.as_str())
            .or_else(|| {
                statement
                    .receiver_symbol
                    .is_valid()
                    .then_some(statement.receiver_symbol)
            })
            .unwrap_or_else(SymbolHandle::invalid);
        crate::flow::push_field_place_segments(program, &mut place.segments, symbol);
    }

    Some(place)
}

fn resolve_self_receiver_member_symbol(
    program: &psi_typed_trees::TypedTrees,
    place: &CanonicalPlace,
    member_name: &str,
) -> Option<SymbolHandle> {
    let type_symbol = symbol_type_symbol(program, canonical_place_root_symbol(place)?)?;
    resolve_member_symbol_from_type_symbol(program, type_symbol, member_name)
}

fn canonical_place_root_symbol(place: &CanonicalPlace) -> Option<SymbolHandle> {
    match place.root {
        psi_facts::PlaceRoot::Symbol(symbol) => Some(symbol),
        _ => None,
    }
}
