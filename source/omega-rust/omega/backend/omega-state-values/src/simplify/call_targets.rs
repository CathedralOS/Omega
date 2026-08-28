use psi_checked_trees::CheckedTrees;
use psi_checked_trees::expression::{CallExpression, Expression};
use psi_checked_trees::machine::Machine;
use psi_symbols::SymbolHandle;

pub(crate) fn resolve_call_target_machine<'program>(
    program: &'program CheckedTrees,
    current_machine: &'program Machine,
    receiver: Option<&Expression>,
    target_symbol: SymbolHandle,
) -> Option<&'program Machine> {
    let Some(receiver) = receiver else {
        return machine_owning_state_symbol(program, target_symbol).or(Some(current_machine));
    };
    let receiver = strip_mutable_expression_ref(receiver);
    if expression_is_self_reference(current_machine, receiver) {
        if let Some(target_machine) = machine_owning_state_symbol(program, target_symbol)
            && machine_can_receive_self_call(current_machine, target_machine)
        {
            return Some(target_machine);
        }
        return Some(current_machine);
    }

    let contained_symbol = match receiver {
        Expression::Member(member)
            if expression_is_self_reference(current_machine, &member.receiver) =>
        {
            member.member_symbol
        }
        Expression::Name(path) => path.symbol(),
        _ => return None,
    };

    if !contained_symbol.is_valid() {
        return None;
    }

    let field_type_symbol = program
        .data_definitions()
        .iter()
        .find(|definition| Some(&definition.name) == current_machine.attached_data.as_ref())
        .and_then(|definition| {
            program
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    psi_checked_trees::data::DataMember::Field(field)
                        if field.symbol == contained_symbol =>
                    {
                        Some(program.type_reference_symbol(field.type_reference))
                    }
                    _ => None,
                })
        })?;
    let field_type = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == field_type_symbol)?;

    program
        .machines()
        .iter()
        .find(|machine| machine.attached_data.as_ref() == Some(&field_type.name))
}

fn machine_owning_state_symbol<'program>(
    program: &'program CheckedTrees,
    state_symbol: SymbolHandle,
) -> Option<&'program Machine> {
    if !state_symbol.is_valid() {
        return None;
    }

    program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == state_symbol)
    })
}

fn machine_can_receive_self_call(current: &Machine, target: &Machine) -> bool {
    current.symbol == target.symbol
        || (current.attached_data.is_some() && current.attached_data == target.attached_data)
}

fn strip_mutable_expression_ref(mut expression: &Expression) -> &Expression {
    while let Expression::Borrow(inner) = expression {
        expression = &inner.target;
    }
    expression
}

pub(crate) fn resolve_call_target_state<'machine>(
    program: &'machine CheckedTrees,
    machine: &'machine Machine,
    call: &CallExpression,
) -> Option<&'machine psi_checked_trees::state::State> {
    if !call.target_symbol.is_valid() {
        return None;
    }

    program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == call.target_symbol)
}

fn expression_is_self_reference(machine: &Machine, expression: &Expression) -> bool {
    match expression {
        Expression::Borrow(inner) => expression_is_self_reference(machine, &inner.target),
        Expression::Name(path) => path.len() == 1 && path.symbol() == machine.symbol,
        _ => false,
    }
}
