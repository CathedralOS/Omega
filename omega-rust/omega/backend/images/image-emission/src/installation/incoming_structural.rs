//! Canonical installed shape of the existing incoming owned-indirect pair ABI.
//! These checks grant no source authority; installation replay also binds the
//! complete retained record to its independently admitted executable image.

use super::{InstallationRecord, InstalledFunction, InstalledInternalUnitCall};
use calling_conventions::{
    CallSignature, CallingPolicy, IndirectPointerLocation, ValueLocation, ValueShape,
    evaluate_call_plan,
};
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
        || !function.unit_continuations.is_empty()
        || function.scalar_affine_cleanup.is_some()
        || !function.scalar_control_affine_cleanups.is_empty()
        || function.ranked_u32_countdown
        || function.scalar_abi.is_some()
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
        || function.unit_call_stacks.len() != calls.len()
        || function.byte_count == 0
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
        stack.stack_alignment == 16
            && stack.frame_bytes.is_multiple_of(8)
            && (calls.is_empty()
                || stack
                    .frame_bytes
                    .checked_add(8)
                    .is_some_and(|bytes| bytes.is_multiple_of(16)))
            && function.unit_call_stacks.iter().all(|call| {
                call.active_frame_bytes == stack.frame_bytes
                    && call.transient_bytes == 8
                    && stack.frame_bytes.checked_add(8) == Some(call.caller_live_bytes)
                    && calls
                        .iter()
                        .filter(|row| {
                            row.custody.owner == call.owner && row.custody.target == call.target
                        })
                        .count()
                        == 1
            })
            && stack.local_peak_bytes
                == function
                    .unit_call_stacks
                    .iter()
                    .map(|call| call.caller_live_bytes)
                    .max()
                    .unwrap_or(stack.frame_bytes)
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
        || call.byte_count != 5
        || !matches!(call.owner, CallSiteOwner::Operation(_))
        || function.text_offset.checked_add(call.code_offset) != Some(installed.text_offset)
        || call
            .code_offset
            .checked_add(call.byte_count)
            .is_none_or(|end| end > function.byte_count)
        || !call_attribution_is_exact(record, function, installed)
    {
        return false;
    }
    let Some(stack) = function
        .unit_call_stacks
        .iter()
        .find(|stack| stack.owner == call.owner && stack.target == call.target)
    else {
        return false;
    };
    if stack.owner != call.owner
        || stack.target != call.target
        || function
            .unit_stack
            .is_none_or(|frame| stack.active_frame_bytes != frame.frame_bytes)
        || stack.transient_bytes != 8
        || stack.active_frame_bytes.checked_add(8) != Some(stack.caller_live_bytes)
        || installed.text_offset.checked_add(1) != Some(stack.text_offset)
    {
        return false;
    }
    for ((argument, home), destination) in call
        .arguments
        .iter()
        .zip(&function.unit_parameter_homes)
        .zip(&callee.unit_parameter_homes)
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
            || argument.source.placement() != Some(&home.source)
            || argument.destination != destination.source
            || argument.call_stack_bytes != stack.active_frame_bytes
            || argument.byte_count == 0
            || argument.bytes.len() != argument.byte_count
            || argument
                .code_offset
                .checked_add(argument.byte_count)
                .is_none_or(|end| end > call.code_offset)
            || !outgoing_fits(&argument.destination, stack.active_frame_bytes)
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
/// Raw installation data checks the relationship among distinct intervals:
/// argument copies precede the call instruction, inside its semantic operation.
/// The complete retained bytes and numeric frame facts are additionally compared
/// against the independently admitted image by validate_installation_record.
pub(super) fn call_attribution_is_exact(
    record: &InstallationRecord,
    function: &InstalledFunction,
    installed: &InstalledInternalUnitCall,
) -> bool {
    let call = &installed.custody;
    let CallSiteOwner::Operation(operation) = call.owner else {
        return false;
    };
    let mut rows = record.semantic_code_attribution.iter().filter(|row| {
        row.machine == function.machine
            && row.attribution.site == machine_code::SemanticCodeSite::Operation(operation)
    });
    let Some(row) = rows.next() else {
        return false;
    };
    if rows.next().is_some() {
        return false;
    }
    let span = row.attribution;
    let Some(call_end) = call.code_offset.checked_add(call.byte_count) else {
        return false;
    };
    let Some(first) = call.arguments.first() else {
        return false;
    };
    span.operation_ordinal == call.operation_ordinal
        && span.code_offset == first.code_offset
        && span.code_offset.checked_add(span.byte_count) == Some(call_end)
        && call_end <= function.byte_count
        && call.arguments.iter().all(|argument| {
            argument.code_offset >= span.code_offset
                && argument.byte_count != 0
                && argument.bytes.len() == argument.byte_count
                && argument
                    .code_offset
                    .checked_add(argument.byte_count)
                    .is_some_and(|end| end <= call.code_offset)
        })
        && call.arguments.windows(2).all(|pair| {
            pair[0]
                .code_offset
                .checked_add(pair[0].byte_count)
                .is_some_and(|end| end <= pair[1].code_offset)
        })
}

fn outgoing_fits(placement: &calling_conventions::ValuePlacement, frame_bytes: u32) -> bool {
    let [
        ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(_),
            copy_stack_byte_offset: Some(offset),
            byte_size,
            alignment,
        },
    ] = placement.locations.as_slice()
    else {
        return false;
    };
    *alignment != 0
        && alignment.is_power_of_two()
        && offset.is_multiple_of(u32::from(*alignment))
        && offset
            .checked_add(u32::from(*byte_size))
            .is_some_and(|end| end <= frame_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_outgoing_copies_require_aligned_space_in_the_retained_frame() {
        let plan = evaluate_call_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature {
                parameters: vec![ValueShape::integer(16, 8); 2],
                result: None,
            },
        )
        .unwrap();
        assert!(
            plan.parameters
                .iter()
                .all(|placement| outgoing_fits(placement, 72))
        );
        assert!(!outgoing_fits(&plan.parameters[1], 63));
        let mut changed = plan.parameters[1].clone();
        let ValueLocation::Indirect {
            copy_stack_byte_offset,
            ..
        } = &mut changed.locations[0]
        else {
            panic!("indirect ABI");
        };
        *copy_stack_byte_offset = Some(u32::MAX - 7);
        assert!(!outgoing_fits(&changed, u32::MAX));
        let ValueLocation::Indirect {
            copy_stack_byte_offset,
            ..
        } = &mut changed.locations[0]
        else {
            panic!("indirect ABI");
        };
        *copy_stack_byte_offset = Some(49);
        assert!(!outgoing_fits(&changed, 80));
    }
}
