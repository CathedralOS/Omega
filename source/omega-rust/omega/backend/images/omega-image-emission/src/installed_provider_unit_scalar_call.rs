//! Source-free custody replay for the exact installed-provider Unit / signed-i32 lane.
//!
//! A forwarded function parameter is deliberately not an ordinary scalar-call
//! source. This validator is the only object boundary that admits that source:
//! it rejoins the caller ABI, selected provider, candidate ABI, native call
//! relocation, and the zero-byte argument interval as one exact occurrence.

use omega_calling_conventions::{
    CallSignature, CallingPolicy, ValueLocation, ValueShape, evaluate_call_plan,
};
use omega_machine_code::{
    InternalUnitScalarArgumentSourceRecord, MachineCodeFunction, UnitScalarParameterLocationRecord,
};
use omega_target::{Architecture, NativeTarget};
use omega_target_operations::{CallSiteOwner, MachineRegister};
use psi_core::{IntegerCarrier, IntegerSign, MachineId};

use super::{ObjectError, ObjectUnitCallStack};

pub(super) fn validate_installed_provider_unit_scalar_calls(
    target: NativeTarget,
    function: &MachineCodeFunction,
    functions: &std::collections::BTreeMap<MachineId, &MachineCodeFunction>,
    validated_call_stacks: &[ObjectUnitCallStack],
) -> Result<(), ObjectError> {
    validate_unit_scalar_abi(target, function)?;
    let invalid = || ObjectError::InvalidInstalledProviderUnitScalarCallEvidence(function.machine);
    if function.installed_provider_unit_scalar_calls.len() > 1 {
        return Err(invalid());
    }
    let mut prior_ordinal = None;
    let mut prior_end = None;
    let mut owners = std::collections::BTreeSet::new();

    for call in &function.installed_provider_unit_scalar_calls {
        let CallSiteOwner::Operation(operation) = call.owner else {
            return Err(invalid());
        };
        let caller_abi = function.unit_scalar_abi.as_ref().ok_or_else(invalid)?;
        let candidate = functions
            .get(&call.provider.candidate)
            .copied()
            .ok_or_else(invalid)?;
        let candidate_abi = candidate.unit_scalar_abi.as_ref().ok_or_else(invalid)?;
        let [caller_parameter] = caller_abi.parameters.as_slice() else {
            return Err(invalid());
        };
        let [candidate_parameter] = candidate_abi.parameters.as_slice() else {
            return Err(invalid());
        };
        let [argument] = call.arguments.as_slice() else {
            return Err(invalid());
        };
        let InternalUnitScalarArgumentSourceRecord::Parameter {
            parameter_index,
            source_value,
            scalar_type,
            location,
        } = argument.source
        else {
            return Err(invalid());
        };
        let expected_register = match target.architecture {
            Architecture::X86_64 => MachineRegister::X86Rdi,
            Architecture::Aarch64 => MachineRegister::Aarch64X(0),
        };
        let expected_location = UnitScalarParameterLocationRecord::Register(expected_register);
        let call_end = call
            .code_offset
            .checked_add(call.byte_count)
            .ok_or_else(invalid)?;
        let matching_relocations = function
            .internal_calls
            .iter()
            .filter(|relocation| {
                relocation.owner == call.owner && relocation.target == call.provider.candidate
            })
            .collect::<Vec<_>>();
        let [relocation] = matching_relocations.as_slice() else {
            return Err(invalid());
        };
        let matching_call_stacks = validated_call_stacks
            .iter()
            .filter(|stack| stack.owner == call.owner && stack.target == call.provider.candidate)
            .collect::<Vec<_>>();
        let [call_stack] = matching_call_stacks.as_slice() else {
            return Err(invalid());
        };
        let exact_attribution_count = function
            .semantic_code_attribution
            .iter()
            .filter(|row| row.site == omega_machine_code::SemanticCodeSite::Operation(operation))
            .filter(|row| row.operation_ordinal == call.operation_ordinal)
            .filter(|row| row.code_offset == call.code_offset && row.byte_count == call.byte_count)
            .count();

        if prior_ordinal.is_some_and(|prior| prior >= call.operation_ordinal)
            || prior_end.is_some_and(|end| end > call.code_offset)
            || !owners.insert(call.owner)
            || call.provider.boundary != call.boundary
            || call.provider.candidate == function.machine
            || call.provider.requirement_identity.is_empty()
            || call.provider.provider_identity.is_empty()
            || call.provider.candidate_identity.is_empty()
            || !call.provider.signature.parameters.is_empty()
            || !call.provider.refinement.positional_parameters.is_empty()
            || call.call_plan != caller_abi.call_plan
            || call.call_plan != candidate_abi.call_plan
            || caller_parameter.scalar_type != candidate_parameter.scalar_type
            || caller_parameter.placement != candidate_parameter.placement
            || parameter_index != 0
            || argument.parameter_index != 0
            || source_value != caller_parameter.value
            || scalar_type != caller_parameter.scalar_type
            || location != expected_location
            || argument.destination != caller_parameter.placement
            || argument.byte_count != 0
            || !call.source_arguments.is_empty()
            || !call.claim_transfers.is_empty()
            || !call.completion_claim_sources.is_empty()
            || !call.completion_receipts.is_empty()
            || call.operation_ordinal != 0
            || function.provenance.operations.as_slice() != [operation]
            || function.provenance.edges.len() != 1
            || !function.provenance.operations.contains(&operation)
            || exact_attribution_count != 1
            || relocation.scalar_stack.is_some()
            || call_stack.text_offset != relocation.offset
        {
            return Err(invalid());
        }
        match target.architecture {
            Architecture::X86_64 => {
                let outbound = relocation
                    .unit_stack
                    .and_then(|stack| stack.outbound)
                    .ok_or_else(invalid)?;
                let allocation_end = outbound
                    .allocation_offset
                    .checked_add(outbound.allocation_byte_count)
                    .ok_or_else(invalid)?;
                let native_call_start = relocation.offset.checked_sub(1).ok_or_else(invalid)?;
                let native_call_end = relocation.offset.checked_add(4).ok_or_else(invalid)?;
                let release_end = outbound
                    .release_offset
                    .checked_add(outbound.release_byte_count)
                    .ok_or_else(invalid)?;
                if outbound.byte_size != 8
                    || outbound.allocation_offset != call.code_offset
                    || allocation_end != argument.code_offset
                    || argument.code_offset != native_call_start
                    || function.bytes.get(native_call_start..native_call_end)
                        != Some(&[0xe8, 0, 0, 0, 0])
                    || outbound.release_offset != native_call_end
                    || release_end != call_end
                {
                    return Err(invalid());
                }
            }
            Architecture::Aarch64 => {
                let native_call_end = relocation.offset.checked_add(4).ok_or_else(invalid)?;
                if relocation
                    .unit_stack
                    .is_none_or(|stack| stack.outbound.is_some())
                    || call.code_offset != relocation.offset
                    || argument.code_offset != relocation.offset
                    || native_call_end != call_end
                    || function.bytes.get(relocation.offset..native_call_end)
                        != Some(&0x9400_0000_u32.to_le_bytes())
                {
                    return Err(invalid());
                }
            }
        }
        prior_ordinal = Some(call.operation_ordinal);
        prior_end = Some(call_end);
    }
    Ok(())
}

fn validate_unit_scalar_abi(
    target: NativeTarget,
    function: &MachineCodeFunction,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidUnitScalarFunctionAbi(function.machine);
    let Some(abi) = function.unit_scalar_abi.as_ref() else {
        return if function.installed_provider_unit_scalar_calls.is_empty() {
            Ok(())
        } else {
            Err(invalid())
        };
    };
    let [parameter] = abi.parameters.as_slice() else {
        return Err(invalid());
    };
    let shape = ValueShape::integer(4, 4);
    let expected = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![shape],
            result: None,
        },
    )
    .map_err(|_| invalid())?;
    let expected_register = match target.architecture {
        Architecture::X86_64 => MachineRegister::X86Rdi,
        Architecture::Aarch64 => MachineRegister::Aarch64X(0),
    };
    if abi.call_plan != expected
        || abi.call_plan.result.is_some()
        || parameter.placement != abi.call_plan.parameters[0]
        || parameter.placement.shape != shape
        || parameter.placement.locations.as_slice()
            != [ValueLocation::Register {
                register: expected_register,
                value_byte_offset: 0,
                byte_size: 4,
            }]
        || parameter.scalar_type.carrier() != IntegerCarrier::Fixed
        || parameter.scalar_type.sign() != IntegerSign::Signed
        || parameter.scalar_type.bits() != 32
        || function.fixed_integer_scalar_abi.is_some()
        || function.scalar_stack.is_some()
        || function.unit_stack.is_none()
        || !function.unit_parameters.is_empty()
        || !function.unit_parameter_homes.is_empty()
    {
        return Err(invalid());
    }
    Ok(())
}
