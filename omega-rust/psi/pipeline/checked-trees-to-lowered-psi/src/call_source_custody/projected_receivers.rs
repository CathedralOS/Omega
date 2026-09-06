//! Rejoin implicit receiver operands to their exact authored parameter place.

use crate::{CheckedTrees, LoweringError, unsupported};
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::types::TypeReferenceNode;
use checked_trees::{
    CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment,
};
use symbols::SymbolHandle;

pub(crate) struct ReceiverSource {
    root: SymbolHandle,
    path: Vec<CheckedUnitStructuralPathSegment>,
    pub(crate) stamp: SymbolHandle,
}

pub(crate) fn source(
    checked: &CheckedTrees,
    caller: SymbolHandle,
    state: SymbolHandle,
    expression: ExpressionHandle,
) -> Result<ReceiverSource, LoweringError> {
    let (machine, state) = crate::scalar_source_custody::authored_state(checked, state)?;
    if machine.symbol != caller {
        return unsupported("projected receiver has a different authored caller");
    }
    let mut cursor = expression;
    let mut path = Vec::new();
    let mut visited = Vec::new();
    let mut stamp = SymbolHandle::invalid();
    let root = loop {
        if !checked.expression_table.expression_is_valid(cursor) || visited.contains(&cursor) {
            return unsupported("projected receiver has a stale or cyclic source");
        }
        visited.push(cursor);
        match checked.expression_table.expression(cursor) {
            ExpressionNode::Member(member)
                if member.case_variant.is_none()
                    && (cursor != expression || member.member_symbol.is_valid()) =>
            {
                let field = validation::exact_self_field(&checked.typed, machine, cursor)
                    .or_else(|| {
                        let reference = validation::declared_place_type_raw(
                            &checked.typed,
                            machine,
                            Some(state),
                            member.receiver,
                        )?;
                        let reference =
                            validation::unwrapped_type_reference(&checked.typed, reference)?;
                        let TypeReferenceNode::Named { symbol, .. } =
                            checked.type_reference_table.type_reference(reference)
                        else {
                            return None;
                        };
                        let owner = checked
                            .data_definitions()
                            .iter()
                            .find(|owner| owner.symbol == *symbol)?;
                        validation::exact_data_member_field(
                            &checked.typed,
                            owner,
                            member.member_symbol,
                            member.member.as_str(),
                            None,
                        )
                    })
                    .ok_or(LoweringError::Unsupported(
                        "projected receiver field has no exact declaration",
                    ))?;
                if field.relevance.is_erased() {
                    return unsupported("projected receiver cannot select an erased field");
                }
                if cursor == expression {
                    stamp = field.symbol;
                }
                path.push(field_segment(field));
                cursor = member.receiver;
            }
            ExpressionNode::Indexed(indexed) => {
                let ExpressionNode::Integer(index) =
                    checked.expression_table.expression(indexed.index)
                else {
                    return unsupported("projected receiver requires a literal fixed index");
                };
                let index = index
                    .value_bignum()
                    .and_then(|value| value.to_u64())
                    .ok_or(LoweringError::Unsupported(
                        "projected receiver index exceeds u64",
                    ))?;
                if !validation::place_has_builtin_coordinates(
                    &checked.typed,
                    machine,
                    Some(state),
                    cursor,
                ) {
                    return unsupported("projected receiver index has no builtin address meaning");
                }
                let reference = validation::declared_place_type_raw(
                    &checked.typed,
                    machine,
                    Some(state),
                    indexed.collection,
                )
                .and_then(|reference| {
                    validation::unwrapped_type_reference(&checked.typed, reference)
                })
                .ok_or(LoweringError::Unsupported(
                    "projected receiver index has no declared collection",
                ))?;
                let TypeReferenceNode::FixedArray {
                    length: checked_trees::types::FixedArrayLength::Literal(length),
                    ..
                } = checked.type_reference_table.type_reference(reference)
                else {
                    return unsupported("projected receiver index has no literal array length");
                };
                if usize::try_from(index)
                    .ok()
                    .is_none_or(|index| index >= *length)
                {
                    return unsupported("projected receiver index is out of bounds");
                }
                path.push(CheckedUnitStructuralPathSegment::FixedIndex(index));
                cursor = indexed.collection;
            }
            ExpressionNode::Name(name)
                if name.symbol.is_valid()
                    && name.head_symbol == name.symbol
                    && checked
                        .expression_table
                        .name_path_members(name.members)
                        .len()
                        == 1 =>
            {
                if let Some(parameter) = checked.state_parameters(state).iter().find(|parameter| {
                    parameter.symbol == name.symbol
                        || (parameter.is_self && name.symbol == machine.symbol)
                }) {
                    break parameter.symbol;
                }
                let field = validation::exact_attached_field(
                    &checked.typed,
                    machine,
                    name.symbol,
                    checked.symbols.name(name.symbol),
                )
                .ok_or(LoweringError::Unsupported(
                    "projected receiver has no exact parameter root",
                ))?;
                if field.relevance.is_erased() {
                    return unsupported("projected receiver cannot select an erased root field");
                }
                path.push(field_segment(field));
                break checked
                    .state_parameters(state)
                    .iter()
                    .find(|parameter| parameter.is_self)
                    .ok_or(LoweringError::Unsupported(
                        "projected receiver has no borrowed self",
                    ))?
                    .symbol;
            }
            _ => {
                return unsupported(
                    "projected receiver is not a content-independent parameter place",
                );
            }
        }
    };
    path.reverse();
    Ok(ReceiverSource { root, path, stamp })
}

fn field_segment(field: &checked_trees::data::DataField) -> CheckedUnitStructuralPathSegment {
    CheckedUnitStructuralPathSegment::Field(
        field
            .identity
            .map(|identity| format!("#{identity}"))
            .unwrap_or_else(|| field.name.as_str().to_owned()),
    )
}

pub(crate) fn validate(
    checked: &CheckedTrees,
    caller: &CheckedUnitEffectMachinePlan,
    operation: &CheckedUnitEffectOperationPlan,
    target: &CheckedUnitEffectMachinePlan,
) -> Result<(), LoweringError> {
    let CheckedUnitEffectOperationPlan::CallUnit {
        coordinate,
        structural_arguments,
        ..
    } = operation
    else {
        return Ok(());
    };
    let authored = super::authored::locate_source(checked, caller.state, *coordinate)?;
    let Some(checked_trees::NominalMachineUseSite::Expression(expression)) = authored.source_site
    else {
        return Ok(());
    };
    let ExpressionNode::Call(call) = checked.expression_table.expression(expression) else {
        return unsupported("receiver call lost its authored expression");
    };
    if !matches!(
        checked.expression_table.expression(call.receiver),
        ExpressionNode::Indexed(_) | ExpressionNode::Member(_)
    ) {
        return Ok(());
    }
    let mut targets = target
        .structural_parameters
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter.is_self);
    let Some((index, target)) = targets.next() else {
        // An unused receiver may be erased by the existing attachment planner.
        // No implicit operand remains to grant storage or borrow authority.
        return Ok(());
    };
    let source = source(checked, caller.machine, caller.state, call.receiver)?;
    let (_, state) = crate::scalar_source_custody::authored_state(checked, caller.state)?;
    let position = checked
        .state_parameters(state)
        .iter()
        .position(|parameter| parameter.symbol == source.root)
        .ok_or(LoweringError::Unsupported(
            "projected receiver lost its source parameter",
        ))?;
    let parameter = caller
        .structural_parameters
        .iter()
        .position(|parameter| parameter.position as usize == position)
        .ok_or(LoweringError::Unsupported(
            "projected receiver lost its retained parameter",
        ))?;
    let argument = structural_arguments
        .get(index)
        .ok_or(LoweringError::Unsupported(
            "projected receiver operand is absent",
        ))?;
    if targets.next().is_some()
        || argument.source_parameter_index() != u32::try_from(parameter).ok()
        || argument.path != source.path
        || argument.type_identity != target.type_identity
        || argument.access != target.access
    {
        return unsupported("projected receiver operand disagrees with its authored place");
    }
    Ok(())
}
