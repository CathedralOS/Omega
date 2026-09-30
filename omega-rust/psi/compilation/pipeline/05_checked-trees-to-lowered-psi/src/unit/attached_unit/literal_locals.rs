//! A literal local establishes byte storage at its authored declaration.
//! Replay joins the payload, carrier and qualifiers before any storage or
//! local binding is published; no caller's parameter lends it authority.
//!
//! Two carriers qualify. A shared byte view (`let s: &[u8] = "ab";`) lends the
//! literal's image storage. A bounded owner (`let s: [u8; 8] in Utf8 = "ab";`)
//! owns its capacity and a live length, and may be mutable: a domain-qualified fixed byte array is
//! a bounded carrier, never a zero-filled raw array
//! (wiki/spec/language/domains.md, "Byte containers and encoding domains").
//! A raw `[u8; N]` without a domain keeps its exact-length rule and is refused.
use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::{CheckedTrees, CheckedUnitEffectOperationPlan};

pub(crate) fn validate(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<(), LoweringError> {
    let CheckedUnitEffectOperationPlan::EstablishByteSequenceLiteral {
        result,
        bytes,
        qualifications,
    } = operation
    else {
        return unsupported("literal local has no establishment");
    };
    let (_, authored) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    let Some(checked_trees::statement::StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(authored.statement_nodes)
        .get(result.statement_index as usize)
    else {
        return unsupported("literal local lost its authored declaration");
    };
    if !local.symbol.is_valid()
        || !checked
            .expression_table
            .expression_is_valid(local.initial_value)
        || result.multiplicity != language_semantics::Multiplicity::Unrestricted
        || checked.type_multiplicity(local.type_reference) != result.multiplicity
    {
        return unsupported("literal local changed its binding custody");
    }
    // Domain identities are source custody, not a producer-chosen list. In
    // particular, dropping Utf8 would permit later writes to erase its invariant.
    let mut declared = Vec::new();
    let mut reference = local.type_reference;
    loop {
        match checked.type_reference_table.type_reference(reference) {
            checked_trees::types::TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                for constraint in checked.type_reference_table.constraints(*constraints) {
                    match constraint {
                        checked_trees::types::TypeConstraintNode::Domain(domain)
                            if domain.semantic_id.is_valid() =>
                        {
                            declared.push(domain.semantic_id)
                        }
                        checked_trees::types::TypeConstraintNode::ArithmeticDomain(_) => {}
                        _ => return unsupported("literal local has an unsupported qualification"),
                    }
                }
                reference = *base_type;
            }
            checked_trees::types::TypeReferenceNode::Reference { referee, .. } => {
                reference = *referee
            }
            _ => break,
        }
    }
    declared.sort_by_key(|domain| domain.0);
    declared.dedup();
    if &declared != qualifications {
        return unsupported("literal local changed its authored qualifications");
    }
    let reference = super::parameters::structural_carrier_type(checked, local.type_reference)?;
    if let Some(capacity) =
        validation::bounded_byte_buffer_capacity(&checked.typed, local.type_reference)
    {
        if bytes.len() > capacity
            || checked.normalized_type_identity(reference).as_str() != result.type_identity
            || !matches!(checked.expression_table.expression(local.initial_value), checked_trees::expression::ExpressionNode::String(authored) if authored.as_ref() == bytes.as_slice())
        {
            return unsupported("owned literal local changed its authored bytes or carrier");
        }
        return Ok(());
    }
    if local.is_mutable {
        return unsupported("literal view local changed its binding custody");
    }
    let checked_trees::types::TypeReferenceNode::Reference {
        referee,
        access: language_semantics::ReferenceAccess::Shared,
        ..
    } = checked.type_reference_table.type_reference(reference)
    else {
        return unsupported("literal local is not a shared byte view");
    };
    let carrier = super::parameters::structural_carrier_type(checked, *referee)?;
    let checked_trees::types::TypeReferenceNode::Slice { element_type } =
        checked.type_reference_table.type_reference(carrier)
    else {
        return unsupported("literal local is not a byte slice");
    };
    if checked.primitive_type_reference(*element_type)
        != Some(checked_trees::types::PrimitiveType::U8)
        || checked.normalized_type_identity(carrier).as_str() != result.type_identity
        || !matches!(checked.expression_table.expression(local.initial_value), checked_trees::expression::ExpressionNode::String(authored) if authored.as_ref() == bytes.as_slice())
    {
        return unsupported("literal local changed its authored bytes or type");
    }
    Ok(())
}
