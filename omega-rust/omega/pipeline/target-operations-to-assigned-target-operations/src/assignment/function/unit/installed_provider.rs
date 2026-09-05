//! Exact assignment for the bounded scalar-bearing installed-provider lane.

use crate::assignment::shared::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn assign(
    machine: MachineId,
    body: &target_operations::TargetUnitBody,
    target: NativeTarget,
    psi_operation: OperationId,
    boundary: semantic_vocabulary::BoundaryMachineId,
    provider: &terminal_psi::ProviderCandidateConformance,
    call_plan: &calling_conventions::CallPlan,
    scalar_arguments: &[target_operations::TargetUnitScalarCallArgument],
    source_arguments: &[terminal_psi::StructuralArgument],
    arguments: &[target_operations::TargetStructuralArgument],
    claim_transfers: &[terminal_psi::ClaimTransfer],
    completion_claim_sources: &[target_operations::CompletionClaimSource],
    completion_receipts: &[terminal_psi::CompletionReceipt],
) -> Result<AssignedUnitOperation, AssignmentError> {
    let invalid = || AssignmentError::InstalledProviderScalarCallCustodyMismatch {
        machine,
        operation: psi_operation,
        boundary,
    };
    let [argument] = scalar_arguments else {
        return Err(invalid());
    };
    let [parameter] = body.scalar_parameters.as_slice() else {
        return Err(invalid());
    };
    let [body_parameter_placement] = body.call_plan.parameters.as_slice() else {
        return Err(invalid());
    };
    let [call_parameter_placement] = call_plan.parameters.as_slice() else {
        return Err(invalid());
    };
    if !matches!(
        body.operations.as_slice(),
        [
            TargetUnitOperation::InstalledProviderCall {
                psi_operation: actual_operation,
                boundary: actual_boundary,
                scalar_arguments: actual_scalar_arguments,
                ..
            },
            TargetUnitOperation::Return { .. },
        ] if *actual_operation == psi_operation
            && *actual_boundary == boundary
            && actual_scalar_arguments.len() == 1
    ) {
        return Err(invalid());
    }
    let TargetUnitScalarArgumentSource::Parameter {
        parameter_index,
        source_value,
        scalar_type,
    } = argument.source
    else {
        return Err(invalid());
    };
    let semantic_vocabulary::ScalarType::Integer(integer_type) = scalar_type else {
        return Err(invalid());
    };
    let shape = super::scalar_call::fixed_integer_shape(source_value, integer_type)
        .map_err(|_| invalid())?;
    let expected_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![shape],
            result: None,
        },
    )
    .map_err(|_| invalid())?;
    if provider.boundary != boundary
        || provider.candidate == machine
        || parameter_index != 0
        || argument.parameter_index != 0
        || parameter.value != source_value
        || parameter.scalar_type != scalar_type
        || parameter.placement != *body_parameter_placement
        || argument.placement != *call_parameter_placement
        || argument.placement.shape != shape
        || integer_type.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
        || integer_type.sign() != semantic_vocabulary::IntegerSign::Signed
        || integer_type.bits() != 32
        || call_plan != &expected_plan
        || call_plan.result.is_some()
        || !source_arguments.is_empty()
        || !arguments.is_empty()
        || !claim_transfers.is_empty()
        || !completion_claim_sources.is_empty()
        || !completion_receipts.is_empty()
    {
        return Err(invalid());
    }
    let location = match parameter.placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if *byte_size == shape.byte_size => {
            crate::assignment::placement::require_register_architecture(
                source_value,
                *register,
                target.architecture,
            )?;
            AssignedScalarLocation::Register(*register)
        }
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size,
                ..
            },
        ] if *byte_size == shape.byte_size => AssignedScalarLocation::IncomingStack {
            byte_offset: *stack_byte_offset,
        },
        _ => return Err(invalid()),
    };
    Ok(AssignedUnitOperation::InstalledProviderCall {
        psi_operation,
        boundary,
        provider: provider.clone(),
        call_plan: call_plan.clone(),
        scalar_arguments: vec![AssignedUnitScalarCallArgument {
            parameter_index: 0,
            source: AssignedUnitScalarArgumentSource::Parameter {
                parameter_index: 0,
                source_value,
                scalar_type,
                location,
            },
            destination: super::scalar_call::assigned_unit_scalar_destination(
                source_value,
                &argument.placement,
                target,
            )?,
        }],
        source_arguments: source_arguments.to_vec(),
        copies: Vec::new(),
        claim_transfers: claim_transfers.to_vec(),
        completion_claim_sources: completion_claim_sources.to_vec(),
        completion_receipts: completion_receipts.to_vec(),
    })
}
