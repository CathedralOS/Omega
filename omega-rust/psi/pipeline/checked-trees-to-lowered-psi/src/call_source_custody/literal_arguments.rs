//! Exact literal payload custody under the authored scalar/structural partition.

use super::*;
use checked_trees::CheckedStructuralAccess;
use checked_trees::expression::ExpressionNode;

pub(super) fn validate(
    checked: &CheckedTrees,
    caller_machine: symbols::SymbolHandle,
    authored: &authored::AuthoredCall,
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<(), LoweringError> {
    let arguments = match operation {
        CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::ScalarCall {
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::StructuralCall {
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryCall {
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            structural_arguments,
            ..
        } => structural_arguments,
        _ => return Ok(()),
    };
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
    let positions = structural_positions(checked, &signature, arguments.len())?;
    for (position, argument) in positions.into_iter().zip(arguments) {
        let source = authored
            .structural_arguments
            .iter()
            .find(|(source_position, _)| *source_position == position)
            .map(|(_, expression)| checked.expression_table.expression(*expression));
        match (argument.byte_sequence_literal(), source) {
            (Some(bytes), Some(ExpressionNode::String(source)))
                if bytes == source.as_ref()
                    && argument.path.is_empty()
                    && argument.access == CheckedStructuralAccess::SharedBorrow => {}
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

pub(crate) fn structural_positions(
    checked: &CheckedTrees,
    signature: &authored::TargetSignature<'_>,
    argument_count: usize,
) -> Result<Vec<u32>, LoweringError> {
    let mut positions = signature
        .parameters
        .iter()
        .enumerate()
        .filter(|(_, parameter)| {
            checked
                .primitive_type_reference(parameter.type_reference)
                .is_none()
        })
        .map(|(position, _)| {
            u32::try_from(position).map_err(|_| {
                LoweringError::Unsupported("literal source parameter position exceeds u32")
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if positions.len() != argument_count {
        // Existing Unit families can erase an unused borrowed receiver. Its
        // separate custody validation decides that erasure; either form must
        // preserve every explicit byte operand's original formal position.
        positions.retain(|position| {
            let parameter = &signature.parameters[*position as usize];
            !parameter.is_self
                || !matches!(
                    checked
                        .type_reference_table
                        .type_reference(parameter.type_reference),
                    checked_trees::types::TypeReferenceNode::Reference { .. }
                )
        });
    }
    if positions.len() != argument_count {
        return unsupported("literal call source lost its structural argument partition");
    }
    Ok(positions)
}
