//! Exact literal payload custody under the authored scalar/structural partition.
use super::{CheckedTrees, CheckedUnitEffectOperationPlan, LoweringError, authored, unsupported};
use checked_trees::CheckedStructuralAccess;
use checked_trees::expression::ExpressionNode;

pub(super) fn validate(
    checked: &CheckedTrees,
    caller_machine: symbols::SymbolHandle,
    authored: &authored::AuthoredCall,
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<(), LoweringError> {
    let Some(planned) = operation.call() else {
        return Ok(());
    };
    let arguments = &planned.structural_arguments;
    if !arguments
        .iter()
        .any(|argument| argument.byte_sequence_literal().is_some())
        && !authored.structural_arguments.iter().any(|(_, expression)| {
            matches!(
                checked.expression_table.expression(*expression),
                ExpressionNode::String(_)
            )
        })
    {
        return Ok(());
    }
    let signature = authored::target_signature(checked, caller_machine, authored.source_target)?;
    let positions = crate::unit::attached_unit::call_parameters::structural_positions(
        checked, planned, &signature,
    )?;
    for (position, argument) in positions.into_iter().zip(arguments) {
        let formal = signature
            .parameters
            .get(position as usize)
            .ok_or(LoweringError::Unsupported("literal call formal is absent"))?;
        // Peel constraints, never references: a referenced bounded buffer is
        // still a loan and cannot authorize constructing an owned argument.
        let formal_carrier =
            crate::unit::attached_unit::structural_carrier_type(checked, formal.type_reference)?;
        let owned_literal = argument.access == CheckedStructuralAccess::Owned
            && matches!(
                checked.type_reference_table.type_reference(formal_carrier),
                checked_trees::types::TypeReferenceNode::FixedArray { .. }
            )
            && validation::bounded_byte_buffer_capacity(&checked.typed, formal.type_reference)
                .is_some_and(|capacity| {
                    argument
                        .byte_sequence_literal()
                        .is_some_and(|bytes| bytes.len() <= capacity)
                })
            && checked.type_multiplicity(formal.type_reference)
                == language_semantics::Multiplicity::Unrestricted
            && checked.normalized_type_identity(formal_carrier).as_str() == argument.type_identity;
        let source = authored
            .structural_arguments
            .iter()
            .find(|(source_position, _)| *source_position == position)
            .map(|(_, expression)| checked.expression_table.expression(*expression));
        match (argument.byte_sequence_literal(), source) {
            (Some(bytes), Some(ExpressionNode::String(source)))
                if bytes == source.as_ref()
                    && argument.path.is_empty()
                    && (argument.access == CheckedStructuralAccess::SharedBorrow
                        || owned_literal) => {}
            (Some(_), _) | (_, Some(ExpressionNode::String(_))) => {
                return unsupported(
                    "literal call argument differs from its authored bytes or access",
                );
            }
            // An implicit receiver has no explicit expression. It cannot stand
            // in for a literal; its own receiver source checks remain separate.
            _ => {}
        }
    }
    Ok(())
}
