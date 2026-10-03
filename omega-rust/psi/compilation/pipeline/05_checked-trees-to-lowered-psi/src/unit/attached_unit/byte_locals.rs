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
    let CheckedUnitEffectOperationPlan::EstablishByteSequence {
        result,
        value,
        qualifications,
    } = operation
    else {
        return unsupported("literal local has no establishment");
    };
    let (owner, authored) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    let statements = checked.statement_table.statements(authored.statement_nodes);
    let (reference, expression, is_mutable, _local) = match statements
        .get(result.statement_index as usize)
    {
        Some(checked_trees::statement::StatementNode::LocalData(local))
            if local.symbol.is_valid() =>
        {
            (
                local.type_reference,
                local.initial_value,
                local.is_mutable,
                true,
            )
        }
        Some(checked_trees::statement::StatementNode::Expression(_))
            if result.statement_index as usize + 1 != statements.len() =>
        {
            return unsupported("literal return is not the authored completion");
        }
        _ => {
            let expression = crate::expression_preparation::source_custody::guarded_exits::completion_expression(
                checked, authored, result.statement_index as usize,
            )?;
            // A completion literal is either an owned bounded carrier or a
            // shared byte view — the view's literal rides image storage just
            // like a `let s: &[u8] = "ab";` local does.
            let shared_view =
                validation::slice_view_shell(&checked.typed, authored.return_type, &[])
                    .is_some_and(|view| view.access == language_semantics::ReferenceAccess::Shared);
            if !shared_view
                && (validation::bounded_byte_buffer_capacity(&checked.typed, authored.return_type)
                    .is_none()
                    || !validation::has_plain_owned_contents_with_represented_constraints(
                        &checked.typed,
                        authored.return_type,
                    ))
            {
                return unsupported("literal completion is not an owned bounded byte result");
            }
            (authored.return_type, expression, false, false)
        }
    };
    if !checked.expression_table.expression_is_valid(expression)
        || result.multiplicity != language_semantics::Multiplicity::Unrestricted
        || checked.type_multiplicity(reference) != result.multiplicity
    {
        return unsupported("literal local changed its binding custody");
    }
    // Domain identities are source custody, not a producer-chosen list. In
    // particular, dropping Utf8 would permit later writes to erase its invariant.
    let mut declared = Vec::new();
    let declared_reference = reference;
    let mut reference = reference;
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
    let reference = super::parameters::structural_carrier_type(checked, declared_reference)?;
    if validation::bounded_byte_buffer_capacity(&checked.typed, declared_reference).is_some() {
        if checked.normalized_type_identity(reference).as_str() != result.type_identity {
            return unsupported("owned byte value changed its authored carrier");
        }
        crate::emission::structural_byte_sequence_store::validate_value(
            checked,
            owner.symbol,
            state,
            result.statement_index as usize,
            expression,
            value,
            value.root,
            0,
        )?;
        return Ok(());
    }
    let bytes = value.literal_bytes().ok_or(LoweringError::Unsupported(
        "borrowed byte local requires literal storage",
    ))?;
    if is_mutable {
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
        || !matches!(checked.expression_table.expression(expression), checked_trees::expression::ExpressionNode::String(authored) if authored.as_ref() == bytes)
    {
        return unsupported("literal local changed its authored bytes or type");
    }
    Ok(())
}
