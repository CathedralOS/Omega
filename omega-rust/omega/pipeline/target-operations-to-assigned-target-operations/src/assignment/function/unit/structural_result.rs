//! Exact ABI and ownership custody for whole affine structural-result calls.

use super::structural_scalar::{declaration_map, structural_value_shape};
use crate::assignment::shared::*;
use semantic_vocabulary::{ScalarType, StructuralTypeId};
use target_operations::{TargetStructuralArgument, TargetUnitBody, TargetUnitScalarCallArgument};
use terminal_psi::{ClaimTransfer, CrashRouteBucket, StructuralFieldType, StructuralTypeShape};

#[allow(clippy::too_many_arguments)]
pub(super) fn assign_result_call(
    machine: MachineId,
    _attachment: Option<StructuralTypeId>,
    body: &TargetUnitBody,
    target: NativeTarget,
    psi_operation: OperationId,
    result: &terminal_psi::StructuralOperationResult,
    result_home: Option<&target_operations::TargetStructuralHomeRequirement>,
    callee: MachineId,
    callee_result: &terminal_psi::StructuralResultDeclaration,
    call_plan: &calling_conventions::CallPlan,
    scalar_arguments: &[TargetUnitScalarCallArgument],
    arguments: &[TargetStructuralArgument],
    claim_transfers: &[ClaimTransfer],
    returned_claim_transfers: &[terminal_psi::StructuralResultClaimTransfer],
    requirement_obligations: &[semantic_vocabulary::ObligationId],
    crash_continuations: &[CrashRouteBucket],
    preceding_operations: &[TargetUnitOperation],
    assigned_scalar_homes: &BTreeMap<ValueId, AssignedUnitScalarHome>,
    assigned_structural_homes: &mut BTreeMap<PlaceId, AssignedStructuralHome>,
    next_home: &mut u32,
) -> Result<AssignedUnitOperation, AssignmentError> {
    let invalid = || AssignmentError::StructuralScalarCallCustodyMismatch {
        machine,
        operation: psi_operation,
    };
    let [argument] = arguments else {
        return Err(invalid());
    };
    let declarations = declaration_map(&body.structural_types).ok_or_else(invalid)?;
    let Some(declaration) = declarations.get(&result.structural_type).copied() else {
        return Err(invalid());
    };
    let exact_record = matches!(
        &declaration.shape,
        StructuralTypeShape::Record { fields }
            if matches!(
                fields.as_slice(),
                [field]
                    if matches!(
                        field.field_type,
                        StructuralFieldType::Scalar(ScalarType::Integer(integer))
                            if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                                && integer.bits() == 64
                    )
            )
    );
    let exact_carrier = match scalar_arguments.len() {
        0 => {
            matches!(declaration.shape, StructuralTypeShape::Record { .. })
                || matches!(declaration.shape, StructuralTypeShape::FixedArray { length, .. } if length > 0)
        }
        1 => exact_record && argument.shape == ValueShape::integer(8, 8),
        _ => false,
    };
    let exact_shape = structural_value_shape(result.structural_type, &declarations);
    let scalar_shapes = scalar_arguments
        .iter()
        .map(|argument| argument.placement.shape)
        .collect::<Vec<_>>();
    let expected_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: scalar_shapes
                .iter()
                .copied()
                .chain([argument.shape])
                .collect(),
            result: Some(argument.shape),
        },
    )
    .map_err(|_| invalid())?;
    let root = body
        .parameters
        .iter()
        .find(|parameter| parameter.place == argument.place)
        .ok_or_else(invalid)?;
    if !exact_carrier
        || exact_shape != Some(argument.shape)
        || expected_plan != *call_plan
        || call_plan.parameters.len() != scalar_arguments.len() + 1
        || scalar_arguments.iter().enumerate().any(|(index, scalar)| {
            usize::try_from(scalar.parameter_index) != Ok(index)
                || call_plan.parameters.get(index) != Some(&scalar.placement)
        })
        || call_plan.parameters.last() != Some(&argument.destination)
        || call_plan.result.as_ref().map(|placement| placement.shape) != Some(argument.shape)
        || result.place == argument.place
        || result.structural_type != argument.structural_type
        || result.structural_type != callee_result.structural_type
        || result.multiplicity != terminal_psi::StructuralMultiplicity::Affine
        || callee_result.multiplicity != terminal_psi::StructuralMultiplicity::Affine
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || !callee_result.qualifications.is_empty()
        || !callee_result.projected_qualifications.is_empty()
        || root.structural_type != argument.root_structural_type
        || root.structural_type != argument.structural_type
        || root.shape != argument.shape
        || root.multiplicity != terminal_psi::StructuralMultiplicity::Affine
        || root.access != terminal_psi::StructuralAccess::Owned
        || !root.projected_qualifications.is_empty()
        || !argument.path.is_empty()
        || argument.access != terminal_psi::StructuralAccess::Owned
        || argument.source_byte_offset != 0
        || argument.fixed_array_length.is_some()
        || argument.element_stride.is_some()
        || argument.source != root.placement
        || !claim_transfers.is_empty()
        || !returned_claim_transfers.is_empty()
        || !requirement_obligations.is_empty()
        || !crash_continuations.is_empty()
    {
        return Err(invalid());
    }
    for placement in [
        &argument.destination,
        call_plan.result.as_ref().ok_or_else(invalid)?,
    ] {
        crate::assignment::placement::validate_direct_structural_return_placement(
            result.place,
            placement,
            target.architecture,
        )
        .map_err(|_| invalid())?;
    }
    let assigned_scalar_arguments = scalar_arguments
        .iter()
        .map(|scalar_argument| {
            super::scalar_call::validate_placement_registers(
                scalar_argument.source.source_value(),
                &scalar_argument.placement,
                target,
            )
            .map_err(|_| invalid())?;
            let source = super::scalar_call::assign_known_unit_scalar_source(
                scalar_argument.source,
                preceding_operations,
                assigned_scalar_homes,
            )
            .ok_or_else(invalid)?;
            Ok(AssignedUnitScalarCallArgument {
                parameter_index: scalar_argument.parameter_index,
                source,
                destination: super::scalar_call::assigned_unit_scalar_destination(
                    scalar_argument.source.source_value(),
                    &scalar_argument.placement,
                    target,
                )
                .map_err(|_| invalid())?,
            })
        })
        .collect::<Result<Vec<_>, AssignmentError>>()?;
    let result_home = result_home
        .map(|requirement| {
            if requirement.result != *result
                || requirement.layout
                    != target_operations::TargetStructuralHomeLayout::Aggregate(argument.shape)
                || call_plan.result.as_ref().is_none_or(|placement| {
                    placement.locations.iter().any(|location| {
                        !matches!(
                            location,
                            ValueLocation::Register {
                                byte_size: 1..=8,
                                ..
                            }
                        )
                    })
                })
                || !scalar_arguments.is_empty()
                || body
                    .parameters
                    .iter()
                    .any(|parameter| parameter.place == result.place)
            {
                return Err(invalid());
            }
            super::structural_homes::assign(
                psi_operation,
                requirement,
                assigned_structural_homes,
                next_home,
            )
        })
        .transpose()?;
    Ok(AssignedUnitOperation::StructuralResultCall {
        psi_operation,
        result: result.clone(),
        result_home,
        callee,
        callee_result: callee_result.clone(),
        call_plan: call_plan.clone(),
        transport: super::scalar_transport::assign(
            call_plan,
            &assigned_scalar_arguments,
            target,
            super::scalar_transport::CallTransportKind::Mixed,
        )?,
        scalar_arguments: assigned_scalar_arguments,
        copies: vec![AssignedAggregateCopy {
            place: argument.place,
            access: argument.access,
            path: Vec::new(),
            root_structural_type: argument.root_structural_type,
            structural_type: argument.structural_type,
            shape: argument.shape,
            source_byte_offset: 0,
            fixed_array_length: None,
            element_stride: None,
            source: argument.source.clone(),
            destination: argument.destination.clone(),
        }],
        claim_transfers: Vec::new(),
        returned_claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    })
}
