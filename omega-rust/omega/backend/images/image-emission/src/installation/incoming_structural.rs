//! Canonical installed shape of the existing incoming owned-indirect pair ABI.
//! These checks grant no source authority; installation replay also binds the
//! complete retained record to its independently admitted executable image.

use super::{InstallationRecord, InstalledFunction, InstalledInternalUnitCall};
use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use machine_code::{InternalUnitCallSource, StructuralSourceLocation};
use target_operations::{CallSiteOwner, MachineRegister};

pub(super) fn has_incoming(function: &InstalledFunction) -> bool {
    function
        .unit_parameter_homes
        .iter()
        .chain(&function.scalar_structural_parameter_homes)
        .any(|home| {
            matches!(
                home.location,
                StructuralSourceLocation::IncomingIndirectPointer { .. }
            )
        })
}

pub(super) fn function_is_exact(record: &InstallationRecord, function: &InstalledFunction) -> bool {
    let calls = record
        .internal_unit_calls
        .iter()
        .filter(|call| call.machine == function.machine)
        .collect::<Vec<_>>();
    if record.target != target::NativeTarget::uefi_x64()
        || function.unit_parameters.len() != 2
        || function.unit_parameter_homes.len() != 2
        || function.unit_body
        || function.unit_affine_cleanup.is_some()
        || function.scalar_affine_cleanup.is_some()
        || !function.scalar_control_affine_cleanups.is_empty()
        || function.ranked_u32_countdown
        || function.fixed_integer_scalar_abi.is_some()
        || function.mixed_structural_scalar_abi.is_some()
        || function.unit_scalar_abi.is_some()
        || function.structural_call_scalar_return.is_some()
        || !function.scalar_structural_parameters.is_empty()
        || !function.scalar_structural_parameter_homes.is_empty()
        || !function.unit_scalar_homes.is_empty()
        || !function.unit_integer_constants.is_empty()
        || !function.unit_affine_scalar_records.is_empty()
        || !function.unit_structural_scalar_field_stores.is_empty()
        || !function.unit_write_only_primitive_stores.is_empty()
        || !function.scalar_structural_scalar_field_stores.is_empty()
        || function.scalar_stack.is_some()
        || !function.scalar_call_stacks.is_empty()
        || !function.foreign_call_stacks.is_empty()
        || calls.len() > 1
        || function.unit_call_stacks.len() != calls.len()
        || function.byte_count != if calls.is_empty() { 1 } else { 90 }
    {
        return false;
    }
    let Ok(plan) = evaluate_call_plan(
        CallingPolicy::native_for_target(record.target),
        &CallSignature {
            parameters: function
                .unit_parameters
                .iter()
                .map(|parameter| parameter.shape)
                .collect(),
            result: None,
        },
    ) else {
        return false;
    };
    if plan.parameters.len() != 2 {
        return false;
    }
    for (index, ((parameter, home), placement)) in function
        .unit_parameters
        .iter()
        .zip(&function.unit_parameter_homes)
        .zip(&plan.parameters)
        .enumerate()
    {
        let register = [MachineRegister::X86Rcx, MachineRegister::X86Rdx][index];
        if parameter.place != home.place
            || parameter.structural_type != home.structural_type
            || parameter.multiplicity != home.multiplicity
            || parameter.access != home.access
            || parameter.shape != home.shape
            || home.shape != ValueShape::integer(16, 8)
            || home.access != terminal_psi::StructuralAccess::Owned
            || !home.indirect
            || home.source != *placement
            || home.location != (StructuralSourceLocation::IncomingIndirectPointer { register })
        {
            return false;
        }
    }
    if function.unit_parameters[0].place == function.unit_parameters[1].place {
        return false;
    }
    function.unit_stack.is_some_and(|stack| {
        stack.frame_bytes == 0
            && stack.stack_alignment == 16
            && stack.local_peak_bytes == if calls.is_empty() { 0 } else { 80 }
    })
}

pub(super) fn call_is_exact(
    record: &InstallationRecord,
    function: &InstalledFunction,
    installed: &InstalledInternalUnitCall,
) -> bool {
    let call = &installed.custody;
    let Some(callee) = record
        .functions
        .iter()
        .find(|candidate| candidate.machine == call.target)
    else {
        return false;
    };
    if !function_is_exact(record, function)
        || !function_is_exact(record, callee)
        || call.arguments.len() != 2
        || call.result.is_some()
        || call.semantic_result.is_some()
        || call.structural_result.is_some()
        || !call.scalar_arguments.is_empty()
        || call.code_offset != 0
        || call.byte_count != 89
        || !matches!(call.owner, CallSiteOwner::Operation(_))
        || installed.text_offset != function.text_offset
    {
        return false;
    }
    let Some(stack) = function.unit_call_stacks.first() else {
        return false;
    };
    if stack.owner != call.owner
        || stack.target != call.target
        || stack.active_frame_bytes != 0
        || stack.transient_bytes != 80
        || stack.caller_live_bytes != 80
        || function.text_offset.checked_add(81) != Some(stack.text_offset)
    {
        return false;
    }
    for (index, ((argument, home), destination)) in call
        .arguments
        .iter()
        .zip(&function.unit_parameter_homes)
        .zip(&callee.unit_parameter_homes)
        .enumerate()
    {
        if argument.place != home.place
            || argument.root_structural_type != home.structural_type
            || argument.structural_type != home.structural_type
            || argument.access != home.access
            || argument.structural_type != destination.structural_type
            || argument.access != destination.access
            || argument.shape != home.shape
            || argument.shape != destination.shape
            || !argument.path.is_empty()
            || argument.source_byte_offset != 0
            || argument.fixed_array_length.is_some()
            || argument.element_stride.is_some()
            || argument.source_location != home.location
            || argument.source != home.source
            || argument.destination != destination.source
            || argument.call_stack_bytes != 72
            || argument.code_offset != 4 + index * 30
            || argument.byte_count != 30
            || argument.bytes != copy_bytes(index)
        {
            return false;
        }
    }
    match &call.source {
        InternalUnitCallSource::Authored => true,
        InternalUnitCallSource::InstalledProvider {
            boundary,
            provider,
            completion_claim_sources,
            completion_receipts,
        } => {
            if *boundary != provider.boundary
                || call.target != provider.candidate
                || provider.requirement_identity.is_empty()
                || provider.provider_identity.is_empty()
                || provider.candidate_identity.is_empty()
                || provider.signature.parameters.len() != 2
                || provider.refinement.positional_parameters.len() != 2
                || !provider
                    .refinement
                    .realized_service_ceiling
                    .windows(2)
                    .all(|pair| pair[0] < pair[1])
            {
                return false;
            }
            if provider
                .signature
                .parameters
                .iter()
                .zip(&callee.unit_parameters)
                .zip(&provider.refinement.positional_parameters)
                .enumerate()
                .any(|(index, ((parameter, target), refinement))| {
                    parameter.position as usize != index
                        || parameter.structural_type != target.structural_type
                        || parameter.multiplicity != target.multiplicity
                        || parameter.access != target.access
                        || refinement.boundary_index as usize != index
                        || refinement.candidate_index as usize != index
                })
            {
                return false;
            }
            let arguments = call
                .arguments
                .iter()
                .map(|argument| terminal_psi::StructuralArgument {
                    place: argument.place,
                    path: argument.path.clone(),
                    access: argument.access,
                })
                .collect::<Vec<_>>();
            call.claim_transfers
                == completion_receipts
                    .iter()
                    .map(|receipt| terminal_psi::ClaimTransfer {
                        claim: receipt.claim,
                        argument_index: receipt.argument_index,
                    })
                    .collect::<Vec<_>>()
                && crate::completion_receipts::completion_receipts_have_exact_custody(
                    &arguments,
                    completion_claim_sources,
                    completion_receipts,
                )
        }
    }
}

fn copy_bytes(index: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    for offset in [0_u32, 8] {
        bytes.extend([0x48, 0x8b, if index == 0 { 0x81 } else { 0x82 }]);
        bytes.extend(offset.to_le_bytes());
        bytes.extend([0x48, 0x89, 0x84, 0x24]);
        bytes.extend((32 + index as u32 * 16 + offset).to_le_bytes());
    }
    bytes
}
