//! Machine emission for the exact Unit-returning installed-provider i32 lane.

use omega_assigned_target_operations::{
    AssignedCallDestination, AssignedScalarLocation, AssignedUnitBody, AssignedUnitOperation,
    AssignedUnitScalarArgumentSource,
};
use omega_calling_conventions::{
    CallSignature, CallingPolicy, ValueLocation, ValueShape, evaluate_call_plan,
};
use omega_machine_code::{
    InstalledProviderUnitScalarCallRecord, InternalCallRelocation,
    InternalUnitScalarArgumentSourceRecord, InternalUnitScalarCallArgumentRecord,
    UnitCallStackEvidence, UnitScalarParameterLocationRecord,
};
use omega_target::{Architecture, NativeTarget};
use omega_target_operations::CallSiteOwner;

use super::super::{EmissionError, emit_x86_64_adjust_sp, stack_adjustment_pair};

pub(super) fn emit_installed_provider_scalar_call(
    bytes: &mut Vec<u8>,
    body: &AssignedUnitBody,
    operation: &AssignedUnitOperation,
    target: NativeTarget,
    operation_ordinal: usize,
    internal_calls: &mut Vec<InternalCallRelocation>,
) -> Result<InstalledProviderUnitScalarCallRecord, EmissionError> {
    let AssignedUnitOperation::InstalledProviderCall {
        psi_operation,
        boundary,
        provider,
        call_plan,
        scalar_arguments,
        source_arguments,
        copies,
        claim_transfers,
        completion_claim_sources,
        completion_receipts,
    } = operation
    else {
        unreachable!("installed-provider emission receives its exact assigned carrier")
    };
    let invalid = || EmissionError::InvalidInstalledProviderScalarCallCustody(*psi_operation);
    let [parameter] = body.scalar_parameters.as_slice() else {
        return Err(invalid());
    };
    let [argument] = scalar_arguments.as_slice() else {
        return Err(invalid());
    };
    let [call_parameter_placement] = call_plan.parameters.as_slice() else {
        return Err(invalid());
    };
    if operation_ordinal != 0
        || !matches!(
            body.operations.as_slice(),
            [
                AssignedUnitOperation::InstalledProviderCall {
                    psi_operation: actual_operation,
                    boundary: actual_boundary,
                    scalar_arguments: actual_scalar_arguments,
                    ..
                },
                AssignedUnitOperation::Return { .. },
            ] if actual_operation == psi_operation
                && actual_boundary == boundary
                && actual_scalar_arguments.len() == 1
        )
    {
        return Err(invalid());
    }
    let AssignedUnitScalarArgumentSource::Parameter {
        parameter_index,
        source_value,
        scalar_type,
        location,
    } = argument.source
    else {
        return Err(invalid());
    };
    let shape = ValueShape::integer(4, 4);
    let expected_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![shape],
            result: None,
        },
    )
    .map_err(|_| invalid())?;
    let source_register = match (
        location,
        parameter.placement.locations.as_slice(),
        target.architecture,
    ) {
        (
            AssignedScalarLocation::Register(location_register),
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size: 4,
                },
            ],
            Architecture::X86_64,
        ) if location_register == *register
            && matches!(register, omega_target_operations::MachineRegister::X86Rdi) =>
        {
            *register
        }
        (
            AssignedScalarLocation::Register(location_register),
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size: 4,
                },
            ],
            Architecture::Aarch64,
        ) if location_register == *register
            && matches!(
                register,
                omega_target_operations::MachineRegister::Aarch64X(0)
            ) =>
        {
            *register
        }
        _ => return Err(invalid()),
    };
    if provider.boundary != *boundary
        || parameter_index != 0
        || argument.parameter_index != 0
        || parameter.value != source_value
        || parameter.scalar_type != psi_core::ScalarType::Integer(scalar_type)
        || parameter.placement.shape != shape
        || scalar_type.carrier() != psi_core::IntegerCarrier::Fixed
        || scalar_type.sign() != psi_core::IntegerSign::Signed
        || scalar_type.bits() != 32
        || call_plan != &expected_plan
        || call_plan.result.is_some()
        || !body.parameters.is_empty()
        || !source_arguments.is_empty()
        || !copies.is_empty()
        || !claim_transfers.is_empty()
        || !completion_claim_sources.is_empty()
        || !completion_receipts.is_empty()
        || argument.destination != AssignedCallDestination::Register(source_register)
    {
        return Err(invalid());
    }

    let code_offset = bytes.len();
    let (argument_code_offset, relocation_offset, outbound_stack) = match target.architecture {
        Architecture::X86_64 => {
            let allocation_offset = bytes.len();
            emit_x86_64_adjust_sp(bytes, 8, false);
            let argument_code_offset = bytes.len();
            bytes.push(0xe8);
            let relocation_offset = bytes.len();
            bytes.extend_from_slice(&0_i32.to_le_bytes());
            let release_offset = bytes.len();
            emit_x86_64_adjust_sp(bytes, 8, true);
            (
                argument_code_offset,
                relocation_offset,
                stack_adjustment_pair(
                    8,
                    Some((allocation_offset, argument_code_offset - allocation_offset)),
                    Some((release_offset, bytes.len() - release_offset)),
                ),
            )
        }
        Architecture::Aarch64 => {
            let argument_code_offset = bytes.len();
            let relocation_offset = bytes.len();
            bytes.extend_from_slice(&0x9400_0000_u32.to_le_bytes());
            (
                argument_code_offset,
                relocation_offset,
                stack_adjustment_pair(0, None, None),
            )
        }
    };
    let argument_record = InternalUnitScalarCallArgumentRecord {
        parameter_index: 0,
        source: InternalUnitScalarArgumentSourceRecord::Parameter {
            parameter_index: 0,
            source_value,
            scalar_type,
            location: UnitScalarParameterLocationRecord::Register(source_register),
        },
        destination: call_parameter_placement.clone(),
        code_offset: argument_code_offset,
        byte_count: 0,
    };
    internal_calls.push(InternalCallRelocation {
        owner: CallSiteOwner::Operation(*psi_operation),
        target: provider.candidate,
        unit_stack: Some(UnitCallStackEvidence {
            outbound: outbound_stack,
        }),
        scalar_stack: None,
        offset: relocation_offset,
    });
    Ok(InstalledProviderUnitScalarCallRecord {
        owner: CallSiteOwner::Operation(*psi_operation),
        boundary: *boundary,
        provider: provider.clone(),
        call_plan: call_plan.clone(),
        arguments: vec![argument_record],
        source_arguments: source_arguments.clone(),
        claim_transfers: claim_transfers.clone(),
        completion_claim_sources: completion_claim_sources.clone(),
        completion_receipts: completion_receipts.clone(),
        operation_ordinal,
        code_offset,
        byte_count: bytes.len() - code_offset,
    })
}
