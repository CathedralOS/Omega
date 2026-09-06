//! Rejoin a write-only receiver with its attached storage declaration.

use super::*;
use typed_trees::data::DataField;
use typed_trees::expression::TableNamePath;
use typed_trees::statement::TableCall;

pub(super) fn record<'program>(
    program: &'program TypedTrees,
    root: &WriteOnlyRoot,
) -> Option<&'program DataDefinition> {
    let machine = machine(program, root)?;
    let definition = closed_write_only_data_by_symbol(program, machine.attached_data_symbol)?;
    (DataDefinition::shape_kind_from_members(program.data_members(definition))
        == DataShapeKind::Record)
        .then_some(definition)
}

pub(super) fn field<'program>(
    program: &'program TypedTrees,
    root: &WriteOnlyRoot,
    expression: ExpressionHandle,
) -> Option<&'program DataField> {
    crate::exact_self_field(program, machine(program, root)?, expression)
}

fn machine<'program>(
    program: &'program TypedTrees,
    root: &WriteOnlyRoot,
) -> Option<&'program Machine> {
    if !root.receiver_machine.is_valid() {
        return None;
    }
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == root.receiver_machine)
}

pub(super) fn validate_state_transfer(
    program: &TypedTrees,
    machine_name: &str,
    state_name: &str,
    target: SymbolHandle,
    roots: &[WriteOnlyRoot],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for root in roots {
        let Some(owner) = machine(program, root) else {
            continue;
        };
        let Some(target_state) = program
            .machine_states(owner)
            .iter()
            .find(|state| state.symbol == target)
        else {
            continue;
        };
        for parameter in program
            .state_parameters(target_state)
            .iter()
            .filter(|parameter| parameter.is_self)
        {
            if !matches!(
                program
                    .type_reference_table
                    .type_reference(parameter.type_reference),
                TypeReferenceNode::Reference {
                    access: ReferenceAccess::WriteOnly,
                    ..
                }
            ) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{machine_name}` state `{state_name}` widens write-only receiver in transition to `{}`; the target receiver must retain `&write` access",
                    target_state.name,
                )));
            }
        }
    }
}

pub(super) fn matches_name(
    program: &TypedTrees,
    root: &WriteOnlyRoot,
    path: &TableNamePath,
) -> bool {
    path.head_symbol == root.symbol
        || (root.receiver_machine.is_valid()
            && path.head_symbol == root.receiver_machine
            && program
                .expression_table
                .name_path_members(path.members)
                .first()
                .is_some_and(|name| name.as_str() == "self"))
}

pub(super) fn mentions_name(
    program: &TypedTrees,
    root: &WriteOnlyRoot,
    path: &TableNamePath,
) -> bool {
    matches_name(program, root, path)
        || program
            .expression_table
            .name_path_members(path.members)
            .first()
            .is_some_and(|name| {
                attached_field(program, root, path.head_symbol, name.as_str()).is_some()
            })
}

pub(super) fn bare_field<'program, 'roots>(
    program: &'program TypedTrees,
    expression: ExpressionHandle,
    roots: &'roots [WriteOnlyRoot],
) -> Option<(&'roots WriteOnlyRoot, &'program DataField)> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    let [name] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    roots.iter().find_map(|root| {
        attached_field(program, root, path.head_symbol, name.as_str()).map(|field| (root, field))
    })
}

fn attached_field<'program>(
    program: &'program TypedTrees,
    root: &WriteOnlyRoot,
    symbol: SymbolHandle,
    name: &str,
) -> Option<&'program DataField> {
    if !symbol.is_valid() {
        return None;
    }
    record(program, root)?;
    crate::places::exact_attached_field(program, machine(program, root)?, symbol, name)
}

pub(super) fn validate_statement_call(
    program: &TypedTrees,
    machine: &str,
    state: &str,
    call: &TableCall,
    roots: &[WriteOnlyRoot],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let receiver = program.statement_table.name_path_members(call.receiver);
    for root in roots {
        if call.receiver_root_symbol == root.symbol
            || (root.receiver_machine.is_valid()
                && call.receiver_root_symbol == root.receiver_machine
                && receiver.first().is_some_and(|name| name.as_str() == "self"))
            || receiver.first().is_some_and(|name| {
                attached_field(program, root, call.receiver_root_symbol, name.as_str()).is_some()
            })
        {
            if receiver.len() == 1
                && (call.receiver_root_symbol == root.symbol
                    || call.receiver_root_symbol == root.receiver_machine)
                && admits_call(program, root, call.target_symbol)
            {
                continue;
            }
            diagnostics.push(Diagnostic::error(format!(
                "machine `{machine}` state `{state}` calls through write-only parameter `{}`; dispatch requires an exact checked `&write self` target on the whole receiver",
                root.name,
            )));
        }
    }
}

/// Dispatch borrows the receiver without loading its contents only when the
/// selected declaration retains write-only access under its exact data owner.
pub(super) fn admits_call(
    program: &TypedTrees,
    root: &WriteOnlyRoot,
    target: SymbolHandle,
) -> bool {
    let Some(owner) = record(program, root).or_else(|| write_only_record(program, root.referee))
    else {
        return false;
    };
    let Some((callee, state)) = crate::calls::machine_state_by_symbol(program, target) else {
        return false;
    };
    if callee.attached_data_symbol != owner.symbol
        || callee.supply_mode != MachineSupplyMode::CheckedBody
    {
        return false;
    }
    let mut receivers = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| parameter.is_self);
    let Some(receiver) = receivers.next() else {
        return false;
    };
    receivers.next().is_none()
        && matches!(
            program
                .type_reference_table
                .type_reference(receiver.type_reference),
            TypeReferenceNode::Reference {
                access: ReferenceAccess::WriteOnly,
                ..
            }
        )
}
