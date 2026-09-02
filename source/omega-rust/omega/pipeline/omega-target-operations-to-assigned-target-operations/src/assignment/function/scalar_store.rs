//! Physical assignment for one direct mutable-self scalar store prefix.

use super::assign_function;
use super::unit::structural_scalar::{declaration_map, direct_integer_field_offset};
use crate::assignment::placement::validate_structural_placement;
use crate::assignment::shared::*;

pub(super) fn assign(
    function: &TargetFunction,
    operation: &TargetOperation,
    target: NativeTarget,
) -> Result<AssignedOperation, AssignmentError> {
    let TargetOperation::ScalarReturnAfterStructuralScalarFieldStore {
        store,
        scalar,
        structural_types,
        call_plan,
        structural_parameters,
    } = operation
    else {
        unreachable!("scalar-store assignment receives its dedicated carrier")
    };
    let invalid = || AssignmentError::StructuralScalarFieldStoreCustodyMismatch {
        machine: function.machine,
        operation: store.psi_operation,
    };
    let declarations = declaration_map(structural_types).ok_or_else(invalid)?;
    let parameter_index = usize::try_from(store.destination.position).map_err(|_| invalid())?;
    let parameter = structural_parameters
        .get(parameter_index)
        .filter(|parameter| {
            parameter.place == store.destination.place
                && parameter.structural_type == store.destination.structural_type
                && parameter.multiplicity == store.destination.multiplicity
                && parameter.access == store.destination.access
                && parameter.projected_qualifications == store.destination.projected_qualifications
                && parameter.placement == store.destination_placement
        })
        .ok_or_else(invalid)?;
    let expected_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: call_plan
                .parameters
                .iter()
                .map(|placement| placement.shape)
                .collect(),
            result: call_plan.result.as_ref().map(|placement| placement.shape),
        },
    )
    .map_err(|_| invalid())?;
    let exact_return = matches!(
        scalar.as_ref(),
        TargetOperation::ReturnIntegerExpression {
            source_value,
            scalar_type,
            expression:
                TargetIntegerExpression::StructuralField {
                    psi_operation,
                    source_value: expression_value,
                    source,
                    field,
                    source_placement,
                    field_byte_offset,
                    integer_type,
                },
            ..
        } if source_value == expression_value
            && scalar_type == &store.scalar_type
            && source == &store.destination.place
            && field == &store.field
            && source_placement == &store.destination_placement
            && field_byte_offset == &store.field_byte_offset
            && integer_type == &store.scalar_type
            && *psi_operation != store.psi_operation
    );
    if !store.destination.is_self
        || function.attachment != Some(store.destination.structural_type)
        || !matches!(
            store.destination.multiplicity,
            psi_terminal::StructuralMultiplicity::Unrestricted
                | psi_terminal::StructuralMultiplicity::Affine
        )
        || store.destination.access != psi_terminal::StructuralAccess::MutableBorrow
        || !store.destination.qualifications.is_empty()
        || !store.destination.projected_qualifications.is_empty()
        || !store.path.is_empty()
        || !store.scalar_type.admits(store.value)
        || direct_integer_field_offset(
            store.destination.structural_type,
            store.field,
            store.scalar_type,
            &declarations,
        ) != Some(store.field_byte_offset)
        || expected_call_plan != *call_plan
        || call_plan.result.is_none()
        || call_plan.parameters.len() < structural_parameters.len()
        || call_plan.parameters[call_plan.parameters.len() - structural_parameters.len()..]
            .iter()
            .zip(structural_parameters)
            .any(|(placement, parameter)| placement != &parameter.placement)
        || !exact_return
    {
        return Err(invalid());
    }
    validate_structural_placement(parameter.place, &parameter.placement, target.architecture)?;
    let assigned_scalar = assign_function(
        &TargetFunction {
            machine: function.machine,
            attachment: function.attachment,
            fixed_integer_scalar_abi: function.fixed_integer_scalar_abi.clone(),
            mixed_structural_scalar_abi: function.mixed_structural_scalar_abi.clone(),
            provenance: function.provenance.clone(),
            operation: scalar.as_ref().clone(),
        },
        target,
        &[],
    )?
    .operation;
    Ok(
        AssignedOperation::ScalarReturnAfterStructuralScalarFieldStore {
            store: store.clone(),
            scalar: Box::new(assigned_scalar),
            structural_types: structural_types.clone(),
            call_plan: call_plan.clone(),
            structural_parameters: structural_parameters.clone(),
        },
    )
}
