//! A record construction replays against its authored struct literal: the
//! nominal carrier is the declared record, the retained field roster covers
//! every declared member exactly once in order, an omitted member is only a
//! zeroed structural leaf, and each field value is either a scalar operand
//! of its declared carrier or a structural value queued for replay.

use super::{
    CheckedScalarExpressionRole, CheckedStructuralValueKind, CheckedTrees, ExpressionHandle,
    ExpressionNode, LoweringError, Replay, SourceArm, SymbolHandle, TypeReferenceHandle,
    unsupported, validate_operand,
};
use arena::HandleSpan;
use checked_trees::CheckedStructuralRecordField;

/// Replay one `Record` node established at `expression` for `reference`.
pub(super) fn validate(
    checked: &CheckedTrees,
    replay: &mut Replay<'_>,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    source_arm: SourceArm,
    data_symbol: SymbolHandle,
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
        return unsupported("record establishment lost its authored constructor");
    };
    let expected = validation::unwrapped_type_reference(&checked.typed, reference)
        .ok_or(LoweringError::Unsupported("record carrier missing"))?;
    // A lifetime-parameterized record names its carrier through a
    // `Generic` node whose type arguments are empty — the authored
    // record name is still the nominal carrier.
    let nominal_carrier = match checked.type_reference_table.type_reference(expected) {
        checked_trees::types::TypeReferenceNode::Named { symbol, .. } => *symbol,
        checked_trees::types::TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } if checked
            .type_reference_table
            .type_reference_handles(*arguments)
            .is_empty() =>
        {
            *base_symbol
        }
        _ => {
            return unsupported("record establishment substituted its nominal carrier");
        }
    };
    if literal.case_name.is_some()
        || literal.type_symbol != data_symbol
        || nominal_carrier != data_symbol
    {
        return unsupported("record establishment substituted its nominal carrier");
    }
    let data = checked
        .data_definitions()
        .iter()
        .find(|data| data.symbol == data_symbol)
        .ok_or(LoweringError::Unsupported("record declaration missing"))?;
    let members = checked.data_members(data);
    if members
        .iter()
        .any(|member| matches!(member, checked_trees::data::DataMember::Variant(_)))
    {
        return unsupported("record establishment selected a sum");
    }
    // Erased members stay in the checked record's field list: they
    // carry semantic content but no runtime storage, so this
    // validation replays their authored initializer and declaration
    // without producing runtime custody or operand work.
    let declared = members
        .iter()
        .filter_map(|member| match member {
            checked_trees::data::DataMember::Field(field) => Some(field),
            _ => None,
        })
        .collect::<Vec<_>>();
    let authored = checked.expression_table.struct_fields(literal.fields);
    let retained = plans
        .record_fields
        .span(fields)
        .ok_or(LoweringError::Unsupported("record field span is stale"))?;
    // The retained roster covers every declared member exactly
    // once; authored initializers name a subset, and each
    // omitted member arrives as a synthesized structural field.
    if retained.len() != declared.len()
        || authored.len() > declared.len()
        || authored.iter().any(|initializer| {
            declared
                .iter()
                .all(|member| member.symbol != initializer.field_symbol)
        })
    {
        return unsupported("record establishment changed its complete field roster");
    }
    let mut selected = Vec::new();
    for (ordinal, field) in retained.iter().enumerate() {
        let declaration = declared
            .iter()
            .find(|item| item.symbol == field.field)
            .ok_or(LoweringError::Unsupported("record field has another owner"))?;
        if selected.contains(&field.field) {
            return unsupported("record establishment reordered or substituted a field");
        }
        selected.push(field.field);
        let Some(initializer) = authored
            .iter()
            .find(|initializer| initializer.field_symbol == field.field)
        else {
            // Omitted members are the planner's zero synthesis:
            // only a structural zeroed-leaf value qualifies, and
            // its own arm replays the declared carrier and the
            // literal-zero element.
            let checked_trees::CheckedStructuralRecordFieldValue::Structural(value) = field.value
            else {
                return unsupported("record establishment omitted a scalar member");
            };
            if !plans.nodes.is_valid(value)
                || !matches!(
                    plans.nodes.get(value).kind,
                    CheckedStructuralValueKind::ZeroedScalarArray { .. }
                )
            {
                return unsupported("record establishment omitted a non-zeroable member");
            }
            operand_roles.push(CheckedScalarExpressionRole::RecordField {
                expression,
                field_ordinal: u32::try_from(ordinal)
                    .map_err(|_| LoweringError::Unsupported("record field ordinal overflow"))?,
            });
            pending.push((
                value,
                field.expression,
                declaration.type_reference,
                source_arm,
                None,
            ));
            continue;
        };
        if field.expression != initializer.value
            || field.type_reference != declaration.type_reference
        {
            return unsupported("record establishment reordered or substituted a field");
        }
        match field.value {
            checked_trees::CheckedStructuralRecordFieldValue::Scalar(value) => {
                let role = CheckedScalarExpressionRole::RecordField {
                    expression,
                    field_ordinal: u32::try_from(ordinal)
                        .map_err(|_| LoweringError::Unsupported("record field ordinal overflow"))?,
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
                    return unsupported("record scalar field changed its carrier");
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
