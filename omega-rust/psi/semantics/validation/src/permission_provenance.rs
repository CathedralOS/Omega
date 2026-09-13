//! Whole-expression origins share one reduction over the semantic operand vocabulary.

use language_semantics::PermissionProvenance;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

#[cfg(test)]
mod tests;

/// Calls, records and arrays retain a common existing operand origin. Operands
/// without an origin do not contribute; differing origins yield no common one.
/// The caller resolves live places and decides how a missing origin establishes
/// a new value. Invalid expression spans and cycles remain errors.
pub fn expression_permission_provenance(
    program: &TypedTrees,
    expression: ExpressionHandle,
    resolve_place: &mut impl FnMut(
        ExpressionHandle,
    ) -> Result<Option<PermissionProvenance>, &'static str>,
) -> Result<Option<PermissionProvenance>, &'static str> {
    expression_origin(program, expression, resolve_place, &mut Vec::new())
}

fn expression_origin(
    program: &TypedTrees,
    expression: ExpressionHandle,
    resolve_place: &mut impl FnMut(
        ExpressionHandle,
    ) -> Result<Option<PermissionProvenance>, &'static str>,
    active: &mut Vec<ExpressionHandle>,
) -> Result<Option<PermissionProvenance>, &'static str> {
    if !program.expression_table.expression_is_valid(expression) || active.contains(&expression) {
        return Err("permission provenance has a stale or cyclic expression");
    }
    let node = program.expression_table.expression(expression);
    if let ExpressionNode::Name(path) = node {
        let mut constants = program
            .const_declarations()
            .iter()
            .filter(|declaration| path.symbol.is_valid() && declaration.symbol == path.symbol);
        if let Some(declaration) = constants.next() {
            if constants.next().is_some()
                || program.symbols.get(path.symbol).kind != symbols::SymbolKind::Const
                || !program
                    .type_reference_table
                    .contains_type_reference(declaration.declared_type)
                || program.type_multiplicity(declaration.declared_type)
                    != language_semantics::Multiplicity::Unrestricted
            {
                return Err("permission provenance constant has invalid declaration custody");
            }
            // Named constants have copying permission and no live owned place.
            return Ok(None);
        }
    }
    if !matches!(
        node,
        ExpressionNode::Call(_)
            | ExpressionNode::StructLiteral(_)
            | ExpressionNode::ArrayLiteral(_)
    ) {
        return resolve_place(expression);
    }
    active.push(expression);
    let mut common = None;
    let mut different = false;
    let mut retain = |operand| -> Result<(), &'static str> {
        if let Some(origin) = expression_origin(program, operand, resolve_place, active)? {
            match common {
                Some(previous) => different |= previous != origin,
                None => common = Some(origin),
            }
        }
        Ok(())
    };
    match node {
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() && !static_namespace_receiver(program, call)? {
                retain(call.receiver)?;
            }
            let arguments = program.expression_table.expression_handles(call.arguments);
            if arguments.len() != call.arguments.len() {
                return Err("permission provenance call arguments are stale");
            }
            for argument in arguments {
                retain(*argument)?;
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            let fields = program.expression_table.struct_fields(literal.fields);
            if fields.len() != literal.fields.len() {
                return Err("permission provenance record fields are stale");
            }
            for field in fields {
                retain(field.value)?;
            }
        }
        ExpressionNode::ArrayLiteral(elements) => {
            let values = program.expression_table.expression_handles(*elements);
            if values.len() != elements.len() {
                return Err("permission provenance array elements are stale");
            }
            for value in values {
                retain(*value)?;
            }
        }
        _ => return Err("permission provenance lost its aggregate expression"),
    }
    active.pop();
    Ok(if different { None } else { common })
}

fn static_namespace_receiver(
    program: &TypedTrees,
    call: &typed_trees::expression::TableCallExpression,
) -> Result<bool, &'static str> {
    if !program.expression_table.expression_is_valid(call.receiver) {
        return Err("permission provenance has a stale call receiver");
    }
    let ExpressionNode::Name(receiver) = program.expression_table.expression(call.receiver) else {
        return Ok(false);
    };
    if !receiver.symbol.is_valid()
        || program.symbols.get(receiver.symbol).kind != symbols::SymbolKind::Data
    {
        return Ok(false);
    }
    // A static attachment such as Region::new is a namespace, not a live
    // operand. Omit it only for its exact selected nonself state; arbitrary
    // unresolved names and value receivers still belong to the place resolver.
    if program
        .data_definitions()
        .iter()
        .filter(|definition| definition.symbol == receiver.symbol)
        .count()
        != 1
    {
        return Err("permission provenance namespace has no unique declaration");
    }
    let mut targets = program.machines().iter().flat_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .filter(move |state| state.symbol == call.target_symbol)
            .map(move |state| (machine, state))
    });
    let Some((machine, state)) = targets.next() else {
        return Err("permission provenance namespace has no selected state");
    };
    if targets.next().is_some()
        || machine.attached_data_symbol != receiver.symbol
        || program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.is_self)
    {
        return Err("permission provenance namespace differs from its selected nonself state");
    }
    Ok(true)
}
