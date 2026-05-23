use super::*;

pub(crate) fn machine_state_count(program: &omega_typed_trees::TypedTrees) -> usize {
    program
        .machines()
        .iter()
        .map(|machine| program.machine_states(machine).len())
        .sum()
}

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
) -> Option<&'a [ProgramName]> {
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
    receiver: Option<&[ProgramName]>,
    _target_state: &ProgramName,
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
        .map(|parameter| machine_symbol_from_type_reference_handle(program, parameter.type_reference))
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
    receiver: Option<&[ProgramName]>,
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
        .map(|parameter| machine_symbol_from_type_reference_handle(program, parameter.type_reference))
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

pub(crate) fn machine_by_symbol(
    program: &omega_typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<&omega_typed_trees::machine::Machine> {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
}

pub(crate) fn machine_symbol_from_type_reference_handle(
    program: &omega_typed_trees::TypedTrees,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
) -> SymbolHandle {
    match program.type_reference_table.type_reference(type_reference) {
        omega_typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            machine_symbol_from_type_reference_handle(program, *referee)
        }
        omega_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            machine_symbol_from_type_reference_handle(program, *base_type)
        }
        omega_typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. }
        | omega_typed_trees::types::TypeReferenceNode::Named {
            symbol: base_symbol,
            ..
        } => *base_symbol,
        omega_typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | omega_typed_trees::types::TypeReferenceNode::Slice { .. }
        | omega_typed_trees::types::TypeReferenceNode::Unit => SymbolHandle::invalid(),
    }
}

pub(crate) fn expression_root_symbol(
    expression: ExpressionHandle,
    expressions: &omega_typed_trees::expression::ExpressionTable,
    machine_symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    match expressions.expression(expression) {
        ExpressionNode::Indexed(indexed) => {
            expression_root_symbol(indexed.collection, expressions, machine_symbol)
        }
        ExpressionNode::Mutable(inner) => {
            expression_root_symbol(*inner, expressions, machine_symbol)
        }
        ExpressionNode::Member(member) => match expressions.expression(member.receiver) {
            ExpressionNode::Name(path)
                if path.members.count() == 1
                    && path.symbol.is_valid()
                    && path.symbol == machine_symbol =>
            {
                member
                    .member_symbol
                    .is_valid()
                    .then_some(member.member_symbol)
            }
            _ => expression_root_symbol(member.receiver, expressions, machine_symbol),
        },
        ExpressionNode::Name(path) => first_valid_name_path_symbol(path, expressions),
        _ => None,
    }
}

pub(crate) fn first_valid_name_path_symbol(
    path: &omega_typed_trees::expression::TableNamePath,
    expressions: &omega_typed_trees::expression::ExpressionTable,
) -> Option<SymbolHandle> {
    expressions
        .name_path_member_symbols(path.member_symbols)
        .first()
        .copied()
        .filter(|symbol| symbol.is_valid())
        .or_else(|| path.head_symbol.is_valid().then_some(path.head_symbol))
        .or_else(|| path.symbol.is_valid().then_some(path.symbol))
}
