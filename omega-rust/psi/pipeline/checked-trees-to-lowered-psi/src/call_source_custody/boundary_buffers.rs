//! Mutable boundary byte operands retain their authored field or borrowed-view root.

use super::{authored, literal_arguments, projected_receivers};
use crate::{CheckedTrees, LoweringError, unsupported};
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
        } => structural_arguments,
        _ => return Ok(()),
    };
    let (machine, state) = crate::scalar_source_custody::authored_state(checked, caller_state)?;
    let signature = authored::target_signature(checked, machine.symbol, call.source_target)?;
    let is_mutable_byte_view = |parameter: &checked_trees::signature::StateParameter| {
        let TypeReferenceNode::Reference {
            access: language_core::ReferenceAccess::Mutable,
            referee,
            ..
        } = checked
            .type_reference_table
            .type_reference(parameter.type_reference)
        else {
            return false;
        };
        let TypeReferenceNode::Slice { element_type } =
            checked.type_reference_table.type_reference(*referee)
        else {
            return false;
        };
        checked.primitive_type_reference(*element_type) == Some(PrimitiveType::U8)
    };
    if !signature.parameters.iter().any(is_mutable_byte_view) {
        return Ok(());
    }
    let positions = literal_arguments::structural_positions(checked, &signature, arguments.len())?;
    for (position, argument) in positions.into_iter().zip(arguments) {
        if !is_mutable_byte_view(&signature.parameters[position as usize]) {
            continue;
        }
        let expression = call
            .structural_arguments
            .iter()
            .find(|(source_position, _)| *source_position == position)
            .map(|(_, expression)| *expression)
            .ok_or(LoweringError::Unsupported(
                "mutable boundary bytes have no authored argument",
            ))?;
        let (target, access) = match checked.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => (borrow.target, borrow.access),
            // Forwarding an existing exclusive view does not require a new
            // borrow expression. The exact source parameter and access checks
            // below still reject owned values and shared views.
            ExpressionNode::Name(_) => (expression, language_core::ReferenceAccess::Mutable),
            _ => return unsupported("mutable boundary bytes lost their authored borrow"),
        };
        let source =
            projected_receivers::store_destination(checked, machine.symbol, state.symbol, target)?;
        let parameter = argument
            .source_parameter_index()
            .and_then(|parameter_index| caller_parameters.get(parameter_index as usize))
            .ok_or(LoweringError::Unsupported(
                "mutable boundary bytes lost their retained source parameter",
            ))?;
        let source_parameter = checked
            .state_parameters(state)
            .get(parameter.position as usize)
            .ok_or(LoweringError::Unsupported(
                "mutable boundary bytes lost their authored source parameter",
            ))?;
        if access != language_core::ReferenceAccess::Mutable
            || argument.access != CheckedStructuralAccess::MutableBorrow
            || source_parameter.symbol != source.root
            || (source.path.is_empty()
                && !is_mutable_byte_view(source_parameter)
                && !(matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { .. })
                    && is_mutable_fixed_byte_array(checked, source_parameter)))
            || source.path != argument.path
        {
            return unsupported(
                "mutable boundary byte argument differs from its authored destination",
            );
        }
    }
    Ok(())
}

fn is_mutable_fixed_byte_array(
    checked: &CheckedTrees,
    parameter: &checked_trees::signature::StateParameter,
) -> bool {
    let TypeReferenceNode::Reference {
        access: language_core::ReferenceAccess::Mutable,
        referee,
        ..
    } = checked
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return false;
    };
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
