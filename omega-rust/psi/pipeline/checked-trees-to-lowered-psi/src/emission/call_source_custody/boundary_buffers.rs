//! Byte loans retain their authored field, array, or borrowed-view root.

use super::{authored, literal_arguments, projected_receivers};
use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use checked_trees::CheckedTrees;
use checked_trees::expression::ExpressionNode;
use checked_trees::types::{PrimitiveType, TypeReferenceNode};
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitEffectOperationPlan, CheckedUnitStructuralParameterPlan,
};
use symbols::SymbolHandle;

pub(super) fn validate(
    checked: &CheckedTrees,
    caller_state: SymbolHandle,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    call: &authored::AuthoredCall,
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<(), LoweringError> {
    let arguments = match operation {
        CheckedUnitEffectOperationPlan::BoundaryCall {
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
        }
        | CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::StructuralCall {
            structural_arguments,
            ..
        } => structural_arguments,
        _ => return Ok(()),
    };
    let (machine, state) =
        crate::expression_preparation::source_custody::authored_state(checked, caller_state)?;
    let signature = authored::target_signature(checked, machine.symbol, call.source_target)?;
    let byte_view_access = |parameter: &checked_trees::signature::StateParameter| {
        let TypeReferenceNode::Reference {
            access, referee, ..
        } = checked
            .type_reference_table
            .type_reference(parameter.type_reference)
        else {
            return None;
        };
        let TypeReferenceNode::Slice { element_type } =
            checked.type_reference_table.type_reference(*referee)
        else {
            return None;
        };
        (checked.primitive_type_reference(*element_type) == Some(PrimitiveType::U8)
            && matches!(
                access,
                language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable
            ))
        .then_some(*access)
    };
    if !signature
        .parameters
        .iter()
        .any(|parameter| byte_view_access(parameter).is_some())
    {
        return Ok(());
    }
    let positions = literal_arguments::structural_positions(checked, &signature, arguments.len())?;
    for (position, argument) in positions.into_iter().zip(arguments) {
        let Some(expected_access) = byte_view_access(&signature.parameters[position as usize])
        else {
            continue;
        };
        let shared = expected_access == language_core::ReferenceAccess::Shared;
        let fixed_window = argument.path.iter().any(|segment| {
            matches!(
                segment,
                checked_trees::CheckedUnitStructuralPathSegment::FixedByteRange { .. }
            )
        });
        let expression = call
            .structural_arguments
            .iter()
            .find(|(source_position, _)| *source_position == position)
            .map(|(_, expression)| *expression)
            .ok_or(LoweringError::Unsupported(
                "boundary byte loan has no authored argument",
            ))?;
        let (target, access) = match checked.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => (borrow.target, borrow.access),
            // Forwarding retains the formal's access; exact source custody
            // below and structural replay still reject access escalation.
            ExpressionNode::Name(_) => (expression, expected_access),
            ExpressionNode::Indexed(_) if shared => (expression, expected_access),
            _ if shared && !fixed_window => continue,
            _ => return unsupported("boundary byte loan lost its authored borrow"),
        };
        let (backing, range) = match checked.expression_table.expression(target) {
            ExpressionNode::Indexed(indexed) => {
                match checked.expression_table.expression(indexed.index) {
                    ExpressionNode::Range(range) => (indexed.collection, Some(range)),
                    _ => (target, None),
                }
            }
            _ => (target, None),
        };
        let backing_type =
            validation::declared_place_type_raw(&checked.typed, machine, Some(state), backing)
                .and_then(|reference| {
                    validation::unwrapped_type_reference(&checked.typed, reference)
                });
        // Existing shared literals and live view descriptors have their own
        // source-custody rules. This owner additionally replays fixed backing
        // and its call window, including the selected range operation.
        if shared
            && !fixed_window
            && !backing_type.is_some_and(|reference| {
                matches!(
                    checked.type_reference_table.type_reference(reference),
                    TypeReferenceNode::FixedArray { .. }
                )
            })
        {
            continue;
        }
        let mut source = projected_receivers::store_destination(
            checked,
            machine.symbol,
            state.symbol,
            None,
            backing,
        )?;
        if let Some(range) = range {
            // Reconstruct bounds from authored source, not the producer's path.
            // This remains a call loan of the backing rather than a store into
            // a synthetic array whose size happens to match the window.
            if range.end_inclusive
                || !validation::has_builtin_subslice_meaning(
                    &checked.typed,
                    machine,
                    Some(state),
                    target,
                )
            {
                return unsupported("byte window lost builtin exclusive-end range meaning");
            }
            let backing_type = backing_type.ok_or(LoweringError::Unsupported(
                "byte window has no declared backing",
            ))?;
            let TypeReferenceNode::FixedArray {
                element_type,
                length: checked_trees::types::FixedArrayLength::Literal(extent),
            } = checked.type_reference_table.type_reference(backing_type)
            else {
                return unsupported("byte window requires fixed-array backing");
            };
            let extent = u64::try_from(*extent)
                .map_err(|_| LoweringError::Unsupported("byte backing extent exceeds u64"))?;
            let endpoint = |expression, omitted| -> Result<u64, LoweringError> {
                if !checked.expression_table.expression_is_valid(expression) {
                    return Ok(omitted);
                }
                let ExpressionNode::Integer(value) =
                    checked.expression_table.expression(expression)
                else {
                    return unsupported("byte window needs independently retained dynamic bounds");
                };
                value.value_bignum().and_then(|value| value.to_u64()).ok_or(
                    LoweringError::Unsupported("byte window endpoint exceeds u64"),
                )
            };
            let start = endpoint(range.start, 0)?;
            let end = endpoint(range.end, extent)?;
            if extent == 0
                || start > end
                || end > extent
                || checked.primitive_type_reference(*element_type) != Some(PrimitiveType::U8)
            {
                return unsupported("byte window exceeds its exact byte backing");
            }
            source.path.push(
                checked_trees::CheckedUnitStructuralPathSegment::FixedByteRange { start, end },
            );
        }
        let parameter = argument
            .source_parameter_index()
            .and_then(|parameter_index| caller_parameters.get(parameter_index as usize))
            .ok_or(LoweringError::Unsupported(
                "boundary byte loan lost its retained source parameter",
            ))?;
        let source_parameter = checked
            .state_parameters(state)
            .get(parameter.position as usize)
            .ok_or(LoweringError::Unsupported(
                "boundary byte loan lost its authored source parameter",
            ))?;
        if access != expected_access
            || argument.access
                != if shared {
                    CheckedStructuralAccess::SharedBorrow
                } else {
                    CheckedStructuralAccess::MutableBorrow
                }
            || source_parameter.symbol != source.root
            || (source.path.is_empty()
                && byte_view_access(source_parameter) != Some(expected_access)
                && !is_fixed_byte_array(checked, source_parameter, shared))
            || source.path != argument.path
        {
            return unsupported("boundary byte loan differs from its authored backing");
        }
    }
    Ok(())
}

fn is_fixed_byte_array(
    checked: &CheckedTrees,
    parameter: &checked_trees::signature::StateParameter,
    shared: bool,
) -> bool {
    let TypeReferenceNode::Reference {
        access, referee, ..
    } = checked
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return false;
    };
    if *access != language_core::ReferenceAccess::Mutable
        && !(shared && *access == language_core::ReferenceAccess::Shared)
    {
        return false;
    }
    let TypeReferenceNode::FixedArray {
        element_type,
        length: checked_trees::types::FixedArrayLength::Literal(_),
    } = checked.type_reference_table.type_reference(*referee)
    else {
        return false;
    };
    matches!(
        checked.type_reference_table.type_reference(*element_type),
        TypeReferenceNode::Named { .. }
    ) && checked.primitive_type_reference(*element_type) == Some(PrimitiveType::U8)
}
