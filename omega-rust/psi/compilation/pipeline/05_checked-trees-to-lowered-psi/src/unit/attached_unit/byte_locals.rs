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
    let CheckedUnitEffectOperationPlan::EstablishByteSequence { result, value, .. } = operation
    else {
        return unsupported("literal local has no establishment");
    };
    let (_, authored) = crate::declarations::state_declaration(checked, state)?;
    let statements = checked.statement_table.statements(authored.statement_nodes);
    let reference = match statements.get(result.statement_index as usize) {
        Some(checked_trees::statement::StatementNode::LocalData(local))
            if local.symbol.is_valid() =>
        {
            local.type_reference
        }
        Some(checked_trees::statement::StatementNode::Expression(_))
            if result.statement_index as usize + 1 != statements.len() =>
        {
            return unsupported("literal return is not the authored completion");
        }
        _ => {
            // A completion literal is either an owned bounded carrier or a
            // shared byte view — the view's literal rides image storage just
            // like a `let s: &[u8] = "ab";` local does.
            let shared_view =
                validation::slice_view_shell(&checked.typed, authored.return_type, &[])
                    .is_some_and(|view| view.access == language_semantics::ReferenceAccess::Shared);
            if !shared_view
                && (crate::content_classes::bounded_byte_capacity(
                    checked,
                    authored.return_type,
                )
                .is_none()
                    || !crate::content_classes::plain_owned_with_represented_constraints(
                        checked,
                        authored.return_type,
                    ))
            {
                return unsupported("literal completion is not an owned bounded byte result");
            }
            authored.return_type
        }
    };
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
                            if domain.semantic_id.is_valid() => {}
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
    let reference = super::parameters::structural_carrier_type(checked, declared_reference)?;
    if crate::content_classes::bounded_byte_capacity(checked, declared_reference).is_some() {
        return Ok(());
    }
    if value.literal_bytes().is_none() {
        return unsupported("borrowed byte local requires literal storage");
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
    let checked_trees::types::TypeReferenceNode::Slice { .. } =
        checked.type_reference_table.type_reference(carrier)
    else {
        return unsupported("literal local is not a byte slice");
    };
    Ok(())
}
