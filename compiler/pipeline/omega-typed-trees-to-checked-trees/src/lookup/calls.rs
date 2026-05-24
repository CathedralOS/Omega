use super::*;
use symbols::{machine_by_symbol, machine_symbol_from_type_reference_handle};

pub(crate) fn statement_call_can_dispatch_to_machine(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    call: &TableCall,
) -> bool {
    resolve_state_call_target(
        program,
        machine,
        state,
        call.receiver_symbol,
        call.target_symbol,
        statement_call_receiver_members(program, call),
        &call.target,
    )
    .is_valid()
        || receiver_can_dispatch_to_machine(
            program,
            machine,
            state,
            call.receiver_symbol,
            statement_call_receiver_members(program, call),
        )
}

pub(crate) fn statement_call_receiver_members<'a>(
    program: &'a omega_typed_trees::TypedTrees,
    call: &TableCall,
) -> Option<&'a [Identifier]> {
    (!call.receiver.is_empty()).then(|| program.statement_table.name_path_members(call.receiver))
}

pub(crate) fn statement_call_receiver_path(
    program: &omega_typed_trees::TypedTrees,
    call: &TableCall,
) -> Option<NamePath> {
    let members = statement_call_receiver_members(program, call)?;

    Some(NamePath::resolved_from_iter(
        members.iter().cloned(),
        call.receiver_symbol,
        call.receiver_symbol,
    ))
}

pub(crate) fn call_receiver_parts(
    program: &omega_typed_trees::TypedTrees,
    receiver: ExpressionHandle,
) -> (
    SymbolHandle,
    Option<omega_checked_trees::expression::NamePath>,
) {
    if !receiver.is_valid() {
        return (SymbolHandle::invalid(), None);
    }

    match program.expression_table.expression(receiver) {
        ExpressionNode::Mutable(inner) => call_receiver_parts(program, *inner),
        ExpressionNode::Name(path) => (
            path.symbol,
            Some(NamePath::resolved_from_iter(
                program
                    .expression_table
                    .name_path_members(path.members)
                    .iter()
                    .cloned(),
                path.head_symbol,
                path.symbol,
            )),
        ),
        ExpressionNode::Member(member) => {
            let (_, path) = call_receiver_parts(program, member.receiver);
            let mut path = path.unwrap_or_default();
            path.push_resolved(member.member.clone(), member.member_symbol);
            (member.member_symbol, Some(path))
        }
        _ => (SymbolHandle::invalid(), None),
    }
}

pub(crate) fn resolve_state_call_target(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    receiver_symbol: SymbolHandle,
    target_symbol: SymbolHandle,
    receiver: Option<&[Identifier]>,
    _target_state: &Identifier,
) -> SymbolHandle {
    if receiver.is_none() || receiver_symbol == machine.symbol {
        return resolve_state_symbol_in_machine(program, machine, target_symbol);
    }

    if !receiver_symbol.is_valid() {
        return SymbolHandle::invalid();
    }

    if let Some(contained) = program
        .machine_contained_objects(machine)
        .iter()
        .find(|contained| contained.symbol == receiver_symbol)
    {
        let Some(target_machine) = machine_by_symbol(program, contained.type_symbol) else {
            return SymbolHandle::invalid();
        };
        return resolve_state_symbol_in_machine(program, target_machine, target_symbol);
    }

    if let Some(target_machine) = machine_by_symbol(program, receiver_symbol) {
        return resolve_state_symbol_in_machine(program, target_machine, target_symbol);
    }

    let type_symbol = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == receiver_symbol)
        .map(|parameter| {
            machine_symbol_from_type_reference_handle(program, parameter.type_reference)
        })
        .unwrap_or_else(SymbolHandle::invalid);
    if type_symbol.is_valid()
        && let Some(target_machine) = machine_by_symbol(program, type_symbol)
    {
        return resolve_state_symbol_in_machine(program, target_machine, target_symbol);
    }

    if target_symbol.is_valid()
        && program
            .machines()
            .iter()
            .flat_map(|machine| program.machine_states(machine).iter())
            .any(|state| state.symbol == target_symbol)
    {
        return target_symbol;
    }

    SymbolHandle::invalid()
}

pub(crate) fn receiver_can_dispatch_to_machine(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    receiver_symbol: SymbolHandle,
    receiver: Option<&[Identifier]>,
) -> bool {
    if receiver.is_none() || receiver_symbol == machine.symbol {
        return true;
    }

    if !receiver_symbol.is_valid() {
        return false;
    }

    if program
        .machine_contained_objects(machine)
        .iter()
        .any(|contained| contained.symbol == receiver_symbol)
    {
        return true;
    }

    let type_symbol = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == receiver_symbol)
        .map(|parameter| {
            machine_symbol_from_type_reference_handle(program, parameter.type_reference)
        })
        .unwrap_or_else(SymbolHandle::invalid);

    machine_by_symbol(program, receiver_symbol).is_some()
        || (type_symbol.is_valid() && machine_by_symbol(program, type_symbol).is_some())
}

fn resolve_state_symbol_in_machine(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state_symbol: SymbolHandle,
) -> SymbolHandle {
    if !state_symbol.is_valid() {
        return SymbolHandle::invalid();
    }

    program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)
        .map(|state| state.symbol)
        .unwrap_or_else(SymbolHandle::invalid)
}
