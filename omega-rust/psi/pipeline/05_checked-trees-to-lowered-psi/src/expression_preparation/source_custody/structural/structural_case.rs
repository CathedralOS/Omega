//! A payload-carrying case construction replays against its authored struct
//! literal: the case belongs to the declared sum, its payload roster is
//! complete and in order, and each payload field is either a scalar operand
//! of its declared carrier or a structural value queued for replay.

use super::{
    CheckedScalarExpressionRole, CheckedTrees, ExpressionHandle, ExpressionNode, LoweringError,
    Replay, SourceArm, SymbolHandle, TypeReferenceHandle, unsupported, validate_operand,
};
use arena::HandleSpan;
use checked_trees::CheckedStructuralRecordField;

/// Replay one `StructuralCase` node established at `expression` for `reference`.
pub(super) fn validate(
    checked: &CheckedTrees,
    replay: &mut Replay<'_>,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    source_arm: SourceArm,
    data_symbol: SymbolHandle,
    case: SymbolHandle,
    fields: HandleSpan<CheckedStructuralRecordField>,
) -> Result<(), LoweringError> {
    let Replay {
        machine,
        state,
        statement_index,
        pending,
        operand_roles,
    } = replay;
    let (machine, state, statement_index) = (*machine, *state, *statement_index);
    let plans = &checked.facts.values.structural_values;
    let ExpressionNode::StructLiteral(literal) = checked.expression_table.expression(expression)
    else {
        return unsupported("case establishment lost its authored constructor");
    };
    let expected = validation::unwrapped_type_reference(&checked.typed, reference)
        .ok_or(LoweringError::Unsupported("case carrier missing"))?;
    if literal.case_symbol != Some(case)
        || literal.type_symbol != data_symbol
        || !matches!(checked.type_reference_table.type_reference(expected), checked_trees::types::TypeReferenceNode::Named { symbol, .. } if *symbol == data_symbol)
    {
        return unsupported("case establishment substituted its nominal owner");
    }
    let data = checked
        .data_definitions()
        .iter()
        .find(|data| data.symbol == data_symbol)
        .ok_or(LoweringError::Unsupported("case owner declaration missing"))?;
    let variant = checked
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            checked_trees::data::DataMember::Variant(variant) if variant.symbol == case => {
                Some(variant)
            }
            _ => None,
        })
        .ok_or(LoweringError::Unsupported(
            "case establishment selected a foreign case",
        ))?;
    let declared = checked.data_payload_fields(variant);
    let authored = checked.expression_table.struct_fields(literal.fields);
    let retained = plans
        .record_fields
        .span(fields)
        .ok_or(LoweringError::Unsupported("case field span is stale"))?;
    if authored.len() != declared.len() || retained.len() != authored.len() {
        return unsupported("case establishment changed its complete payload roster");
    }
    let mut selected = Vec::new();
    for (ordinal, (field, initializer)) in retained.iter().zip(authored).enumerate() {
        let declaration = declared
            .iter()
            .find(|item| item.symbol == field.field)
            .ok_or(LoweringError::Unsupported("case field has another owner"))?;
        if selected.contains(&field.field)
            || field.field != initializer.field_symbol
            || field.expression != initializer.value
            || field.type_reference != declaration.type_reference
        {
            return unsupported("case establishment reordered or substituted a field");
        }
        selected.push(field.field);
        match field.value {
            checked_trees::CheckedStructuralRecordFieldValue::Scalar(value) => {
                let role = CheckedScalarExpressionRole::StructuralValueField {
                    expression,
                    field_ordinal: u32::try_from(ordinal)
                        .map_err(|_| LoweringError::Unsupported("case field ordinal overflow"))?,
                };
                let primitive = validate_operand(
                    checked,
                    machine,
                    state,
                    statement_index,
                    role,
                    value,
                    initializer.value,
                )?;
                let expected = validation::unwrapped_type_reference(
                    &checked.typed,
                    declaration.type_reference,
                )
                .and_then(|reference| checked.primitive_type_reference(reference));
                if validation::reference_result_custody::parts(
                    &checked.typed,
                    declaration.type_reference,
                )
                .is_some()
                    || expected != Some(primitive)
                {
                    return unsupported("case scalar field changed its carrier");
                }
                operand_roles.push(role);
            }
            checked_trees::CheckedStructuralRecordFieldValue::Structural(value) => {
                pending.push((
                    value,
                    initializer.value,
                    declaration.type_reference,
                    source_arm,
                    None,
                ));
            }
        }
    }
    Ok(())
}
