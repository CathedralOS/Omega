use omega_checked_trees::CheckedTrees;
use omega_checked_trees::expression::{CallExpression, Expression};
use omega_checked_trees::machine::Machine;
use omega_core::symbols::SymbolHandle;

pub(super) fn resolve_call_target_machine<'program>(
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

    let contained = program
        .machine_contained_objects(current_machine)
        .iter()
        .find(|contained| contained.symbol == contained_symbol)?;

    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == contained.type_symbol)
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
    while let Expression::Mutable(inner) = expression {
        expression = inner.as_ref();
    }
    expression
}

pub(super) fn resolve_call_target_state<'machine>(
    program: &'machine CheckedTrees,
    machine: &'machine Machine,
    call: &CallExpression,
) -> Option<&'machine omega_checked_trees::state::State> {
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
        Expression::Mutable(inner) => expression_is_self_reference(machine, inner),
        Expression::Name(path) => path.len() == 1 && path.symbol() == machine.symbol,
        _ => false,
    }
}
