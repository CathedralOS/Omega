use super::symbols::{machine_by_symbol, machine_symbol_from_type_reference_handle};
use super::*;

pub(crate) fn statement_call_can_dispatch_to_machine(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
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
    program: &'a typed_trees::TypedTrees,
    call: &TableCall,
) -> Option<&'a [Identifier]> {
    (!call.receiver.is_empty()).then(|| program.statement_table.name_path_members(call.receiver))
}

pub(crate) fn statement_call_receiver_path(
    program: &typed_trees::TypedTrees,
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
    program: &typed_trees::TypedTrees,
    receiver: ExpressionHandle,
) -> (SymbolHandle, Option<checked_trees::expression::NamePath>) {
    if !receiver.is_valid() {
        return (SymbolHandle::invalid(), None);
    }

    match program.expression_table.expression(receiver) {
        ExpressionNode::Borrow(inner) => call_receiver_parts(program, inner.target),
        ExpressionNode::Name(path) => (
            resolve_name_path_member_symbol(
                program,
                path,
                program
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    .saturating_sub(1),
            ),
            Some(NamePath::resolved_from_iter(
                program
                    .expression_table
                    .name_path_members(path.members)
                    .iter()
                    .cloned(),
                path.head_symbol,
                resolve_name_path_member_symbol(
                    program,
                    path,
                    program
                        .expression_table
                        .name_path_members(path.members)
                        .len()
                        .saturating_sub(1),
                ),
            )),
        ),
        ExpressionNode::Member(member) => {
            let (_, path) = call_receiver_parts(program, member.receiver);
            let mut path = path.unwrap_or_default();
            let member_symbol =
                crate::flow::effective_member_symbol(program, member.receiver, member);
            path.push_resolved(member.member.clone(), member_symbol);
            (member_symbol, Some(path))
        }
        _ => (SymbolHandle::invalid(), None),
    }
}

pub(crate) fn resolve_name_path_member_symbol(
    program: &typed_trees::TypedTrees,
    path: &typed_trees::expression::TableNamePath,
    target_index: usize,
) -> SymbolHandle {
    let members = program.expression_table.name_path_members(path.members);
    let authored = program
        .expression_table
        .name_path_member_symbols(path.member_symbols);
    let mut selected = SymbolHandle::invalid();

    for (index, member) in members.iter().enumerate() {
        let mut direct = authored
            .get(index)
            .copied()
            .unwrap_or_else(SymbolHandle::invalid);
        if !direct.is_valid() && index == 0 {
            direct = path.head_symbol;
        }
        if !direct.is_valid() && index + 1 == members.len() {
            direct = path.symbol;
        }
        selected = if direct.is_valid() {
            direct
        } else {
            crate::flow::symbol_type_symbol(program, selected)
                .and_then(|type_symbol| {
                    crate::flow::resolve_member_symbol_from_type_symbol(
                        program,
                        type_symbol,
                        member.as_str(),
                    )
                })
                .unwrap_or_else(SymbolHandle::invalid)
        };
        if index == target_index {
            return selected;
        }
        if !selected.is_valid() {
            break;
        }
    }
    SymbolHandle::invalid()
}

pub(crate) fn resolve_state_call_target(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    receiver_symbol: SymbolHandle,
    target_symbol: SymbolHandle,
    receiver: Option<&[Identifier]>,
    target_state: &Identifier,
) -> SymbolHandle {
    if receiver.is_none() || receiver_symbol == machine.symbol {
        let local = resolve_state_symbol_in_machine(program, machine, target_symbol);
        if local.is_valid() {
            return local;
        }
        if machine_parameter_signature_symbol(program, machine, target_symbol).is_valid() {
            return target_symbol;
        }
        // A receiverless call to a FREE top-level machine: symbol resolution
        // points target_symbol at the free machine's entry state, which lives
        // outside the caller machine.
        return state_symbol_in_any_machine(program, target_symbol);
    }

    if !receiver_symbol.is_valid() {
        return SymbolHandle::invalid();
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
        .or_else(|| receiver_field_type_machine_symbol(program, machine, receiver_symbol))
        .or_else(|| crate::flow::symbol_type_symbol(program, receiver_symbol))
        .unwrap_or_else(SymbolHandle::invalid);
    if type_symbol.is_valid()
        && let Some(target_machine) = machine_by_symbol(program, type_symbol)
    {
        return resolve_state_symbol_in_machine(program, target_machine, target_symbol);
    }

    // A method call on a reference PARAMETER of a DATA type (`s: &mut Circle`,
    // `s.code()`): the param's type is a data type, not a machine, so resolve the
    // method (by name) in the machine ATTACHED to that data type -- the same way a
    // contained-object call resolves. Also covers a devirtualized `dyn Trait` param.
    let attached = attached_machine_state_symbol(program, type_symbol, target_symbol, target_state);
    if attached.is_valid() {
        return attached;
    }

    if state_symbol_in_any_machine(program, target_symbol).is_valid() {
        return target_symbol;
    }

    // A call through a TRAIT-typed receiver (`self.console.show(item)` where
    // `console: Console` and Console is a boundary trait): the resolved target
    // is the trait's machine signature, which carries the requires/ensures
    // contracts the caller must discharge.
    if trait_machine_signature_symbol(program, target_symbol).is_valid() {
        return target_symbol;
    }

    SymbolHandle::invalid()
}

/// `target_symbol` when it denotes a compile-time machine parameter declared
/// by `machine`, or invalid. Such a target is a callable signature during
/// modular checking and becomes a concrete state only during specialization.
pub(crate) fn machine_parameter_signature_symbol(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    target_symbol: SymbolHandle,
) -> SymbolHandle {
    if target_symbol.is_valid()
        && program
            .machine_parameter_signature_in(machine, target_symbol)
            .is_some()
    {
        return target_symbol;
    }

    SymbolHandle::invalid()
}

/// `target_symbol` when it is a state of ANY machine in the program (a free
/// machine's entry state, a method state resolved cross-machine), or invalid.
fn state_symbol_in_any_machine(
    program: &typed_trees::TypedTrees,
    target_symbol: SymbolHandle,
) -> SymbolHandle {
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

/// `target_symbol` when it is a machine signature of ANY trait in the program
/// (the resolved target of a call through a trait-typed receiver), or invalid.
pub(crate) fn trait_machine_signature_symbol(
    program: &typed_trees::TypedTrees,
    target_symbol: SymbolHandle,
) -> SymbolHandle {
    if target_symbol.is_valid()
        && program
            .traits()
            .iter()
            .flat_map(|trait_definition| program.trait_machine_signatures(trait_definition).iter())
            .any(|signature| signature.symbol == target_symbol)
    {
        return target_symbol;
    }

    SymbolHandle::invalid()
}

/// The state symbol of method `target_state` (matched by symbol or NAME) in the
/// machine ATTACHED to data type `data_symbol`, or invalid. Resolves a method call
/// on a data-typed reference receiver (a `&mut Data` param, or a devirtualized
/// `dyn Trait`) to the implementing machine's state.
fn attached_machine_state_symbol(
    program: &typed_trees::TypedTrees,
    data_symbol: SymbolHandle,
    target_symbol: SymbolHandle,
    target_state: &Identifier,
) -> SymbolHandle {
    if !data_symbol.is_valid() {
        return SymbolHandle::invalid();
    }
    let Some(data) = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == data_symbol)
    else {
        return SymbolHandle::invalid();
    };
    for candidate in program
        .machines()
        .iter()
        .filter(|candidate| candidate.attached_data.as_ref() == Some(&data.name))
    {
        for state in program.machine_states(candidate) {
            if (target_symbol.is_valid() && state.symbol == target_symbol)
                || state.name == *target_state
            {
                return state.symbol;
            }
        }
    }
    SymbolHandle::invalid()
}

/// Whether `data_symbol` is a data type that has at least one machine attached to
/// it (so a `&mut Data` receiver can dispatch a method call to that machine).
fn data_type_has_attached_machine(
    program: &typed_trees::TypedTrees,
    data_symbol: SymbolHandle,
) -> bool {
    data_symbol.is_valid()
        && program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == data_symbol)
            .is_some_and(|data| {
                program
                    .machines()
                    .iter()
                    .any(|machine| machine.attached_data.as_ref() == Some(&data.name))
            })
}

pub(crate) fn receiver_can_dispatch_to_machine(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    receiver_symbol: SymbolHandle,
    receiver: Option<&[Identifier]>,
) -> bool {
    if receiver.is_none() || receiver_symbol == machine.symbol {
        return true;
    }

    if !receiver_symbol.is_valid() {
        return false;
    }

    let type_symbol = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == receiver_symbol)
        .map(|parameter| {
            machine_symbol_from_type_reference_handle(program, parameter.type_reference)
        })
        .or_else(|| receiver_field_type_machine_symbol(program, machine, receiver_symbol))
        .unwrap_or_else(SymbolHandle::invalid);

    machine_by_symbol(program, receiver_symbol).is_some()
        || (type_symbol.is_valid() && machine_by_symbol(program, type_symbol).is_some())
        || data_type_has_attached_machine(program, type_symbol)
}

fn receiver_field_type_machine_symbol(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    receiver_symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    program
        .machine_owned_data(machine)
        .iter()
        .find(|owned| owned.symbol == receiver_symbol)
        .map(|owned| machine_symbol_from_type_reference_handle(program, owned.type_reference))
        .or_else(|| {
            attached_data_field_type_reference(program, machine, receiver_symbol).map(
                |type_reference| machine_symbol_from_type_reference_handle(program, type_reference),
            )
        })
        .filter(|symbol| symbol.is_valid())
}

fn attached_data_field_type_reference(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    receiver_symbol: SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let attached_data = machine.attached_data.as_ref()?;
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name == *attached_data)?;

    program.data_members(data).iter().find_map(|member| {
        let typed_trees::data::DataMember::Field(field) = member else {
            return None;
        };
        (field.symbol == receiver_symbol).then_some(field.type_reference)
    })
}

fn resolve_state_symbol_in_machine(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
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
