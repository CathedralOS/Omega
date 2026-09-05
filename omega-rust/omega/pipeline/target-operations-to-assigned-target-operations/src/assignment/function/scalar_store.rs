//! Physical assignment for a bounded direct mutable-self scalar store prefix.

use super::assign_function;
use super::unit::structural_scalar::{
    declaration_map, direct_scalar_field_offset, scalar_field_offset_at_path,
};
use crate::assignment::placement::validate_structural_placement;
use crate::assignment::shared::*;
use semantic_vocabulary::ScalarType;
use target_operations::TargetScalarImmediate;

pub(super) fn assign(
    function: &TargetFunction,
    operation: &TargetOperation,
    target: NativeTarget,
) -> Result<AssignedOperation, AssignmentError> {
    let TargetOperation::ScalarReturnAfterStructuralScalarFieldStores {
        stores,
        scalar,
        structural_types,
        call_plan,
        structural_parameters,
    } = operation
    else {
        unreachable!("scalar-store assignment receives its dedicated carrier")
    };
    let Some(anchor) = stores.first() else {
        return Err(AssignmentError::EmptyStructuralScalarFieldStores(
            function.machine,
        ));
    };
    let invalid = || AssignmentError::StructuralScalarFieldStoreCustodyMismatch {
        machine: function.machine,
        operation: anchor.psi_operation,
    };
    if stores.len() > 3 {
        return Err(invalid());
    }
    let declarations = declaration_map(structural_types).ok_or_else(invalid)?;
    let parameter_index = usize::try_from(anchor.destination.position).map_err(|_| invalid())?;
    let parameter = structural_parameters
        .get(parameter_index)
        .filter(|parameter| {
            parameter.place == anchor.destination.place
                && parameter.structural_type == anchor.destination.structural_type
                && parameter.multiplicity == anchor.destination.multiplicity
                && parameter.access == anchor.destination.access
                && parameter.projected_qualifications == anchor.destination.projected_qualifications
                && parameter.placement == anchor.destination_placement
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
    let exact_return = match scalar.as_ref() {
        TargetOperation::ReturnBooleanExpression {
            source_value,
            expression:
                TargetBooleanExpression::StructuralField {
                    psi_operation,
                    source_value: expression_value,
                    source,
                    field,
                    source_placement,
                    field_byte_offset,
                },
            ..
        } => {
            source_value == expression_value
                && source == &anchor.destination.place
                && source_placement == &anchor.destination_placement
                && direct_scalar_field_offset(
                    anchor.destination.structural_type,
                    *field,
                    ScalarType::Boolean,
                    &declarations,
                ) == Some(*field_byte_offset)
                && stores
                    .iter()
                    .all(|store| *psi_operation != store.psi_operation)
        }
        TargetOperation::ReturnIntegerExpression {
            source_value,
            scalar_type: return_type,
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
        } => {
            source_value == expression_value
                && return_type == integer_type
                && source == &anchor.destination.place
                && source_placement == &anchor.destination_placement
                && direct_scalar_field_offset(
                    anchor.destination.structural_type,
                    *field,
                    ScalarType::Integer(*integer_type),
                    &declarations,
                ) == Some(*field_byte_offset)
                && stores
                    .iter()
                    .all(|store| *psi_operation != store.psi_operation)
        }
        _ => false,
    };
    let valid_stores = stores.iter().enumerate().all(|(index, store)| {
        let valid_immediate = match store.immediate {
            TargetScalarImmediate::Boolean(_) => true,
            TargetScalarImmediate::Integer { scalar_type, value } => scalar_type.admits(value),
        };
        store.destination == anchor.destination
            && store.destination_placement == anchor.destination_placement
            && valid_immediate
            && scalar_field_offset_at_path(
                store.destination.structural_type,
                &store.path,
                store.field,
                store.immediate.scalar_type(),
                &declarations,
            ) == Some(store.field_byte_offset)
            && !stores[..index].iter().any(|earlier| {
                earlier.psi_operation == store.psi_operation
                    || earlier.defining_operation == store.defining_operation
                    || earlier.source_value == store.source_value
                    || (earlier.path == store.path && earlier.field == store.field)
            })
    });
    if !anchor.destination.is_self
        || function.attachment != Some(anchor.destination.structural_type)
        || !matches!(
            anchor.destination.multiplicity,
            terminal_psi::StructuralMultiplicity::Unrestricted
                | terminal_psi::StructuralMultiplicity::Affine
        )
        || anchor.destination.access != terminal_psi::StructuralAccess::MutableBorrow
        || !anchor.destination.qualifications.is_empty()
        || !anchor.destination.projected_qualifications.is_empty()
        || !valid_stores
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
        AssignedOperation::ScalarReturnAfterStructuralScalarFieldStores {
            stores: stores.clone(),
            scalar: Box::new(assigned_scalar),
            structural_types: structural_types.clone(),
            call_plan: call_plan.clone(),
            structural_parameters: structural_parameters.clone(),
        },
    )
}
