use isa_aarch64::encode_aarch64_selected_form;
use isa_x86_64::encode_x86_64_selected_form;
use physical_instructions::PostAllocationMachineInstruction;
use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeKey, MachineEncodedEffects, MachineSizeKnowledge, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind,
};
use target::{Architecture, NativeTarget};

use super::{
    DeferredControlEncodingReason, OptimizedSelectedFormEncodingError,
    SelectedFormDecodedFootprint, SelectedFormEncodingRow, SelectedFormEncodingState,
    SelectedFormMachineDisposition,
};

mod scalar_call;

#[cfg(test)]
mod narrow_load_tests;
#[cfg(test)]
mod tests;

pub(super) fn encode_row(
    target: NativeTarget,
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    address: Option<machine_code::ResolvedPhysicalAddress>,
) -> Result<SelectedFormEncodingRow, OptimizedSelectedFormEncodingError> {
    let architecture = target.architecture;
    let alternative = machine.alternative.key;
    let state = match selected.kind {
        kind @ (SelectedInstructionKind::Store { .. }
        | SelectedInstructionKind::AddressOffset { .. }
        | SelectedInstructionKind::Load64 { .. }
        | SelectedInstructionKind::LoadPacked { .. }
        | SelectedInstructionKind::StorePacked { .. }
        | SelectedInstructionKind::Load8 { .. }
        | SelectedInstructionKind::Load16 { .. }
        | SelectedInstructionKind::Load32 { .. }
        | SelectedInstructionKind::HostedWriteByteI32 { .. }
        | SelectedInstructionKind::HostedReadByte { .. }
        | SelectedInstructionKind::Load8Indexed
        | SelectedInstructionKind::Store64 { .. }
        | SelectedInstructionKind::FrameAddress { .. }) => {
            let address = address.ok_or(OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
            let views = machine
                .operands
                .iter()
                .map(|operand| operand.view)
                .collect::<Vec<_>>();
            if architecture == Architecture::Aarch64 {
                let encoded = if matches!(kind, SelectedInstructionKind::HostedWriteByteI32 { .. })
                {
                    isa_aarch64::encode_aarch64_selected_hosted_write_byte_form(
                        target,
                        physical,
                        kind,
                        alternative,
                        &views,
                        address.displacement,
                    )
                } else if matches!(kind, SelectedInstructionKind::HostedReadByte { .. }) {
                    isa_aarch64::encode_aarch64_selected_hosted_read_byte_form(
                        target,
                        physical,
                        kind,
                        alternative,
                        &views,
                        address.displacement,
                    )
                } else {
                    isa_aarch64::encode_aarch64_selected_memory_form(
                        physical,
                        kind,
                        alternative,
                        &views,
                        address.displacement,
                    )
                }
                .map_err(OptimizedSelectedFormEncodingError::Aarch64)?;
                let footprint = encoded.footprint();
                validate_operand_footprint(
                    selected.id,
                    machine,
                    &footprint.encoded,
                    &footprint.register_reads,
                    &footprint.register_writes,
                )?;
                if footprint.encoded != machine.alternative.encoded {
                    return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
                }
                validate_size(selected.id, machine.alternative.size, encoded.bytes().len())?;
                SelectedFormEncodingState::Encoded {
                    bytes: encoded.bytes().to_vec(),
                    footprint: Box::new(SelectedFormDecodedFootprint {
                        register_reads: footprint.register_reads.clone(),
                        register_writes: footprint.register_writes.clone(),
                        implicit_defs: footprint.encoded.implicit_unit_defs.clone(),
                        implicit_clobbers: footprint.encoded.implicit_unit_clobbers.clone(),
                        encoded: footprint.encoded.clone(),
                    }),
                }
            } else {
                let encode = if matches!(kind, SelectedInstructionKind::HostedWriteByteI32 { .. }) {
                    isa_x86_64::encode_x86_64_selected_hosted_write_byte_form
                } else if matches!(kind, SelectedInstructionKind::HostedReadByte { .. }) {
                    isa_x86_64::encode_x86_64_selected_hosted_read_byte_form
                } else {
                    isa_x86_64::encode_x86_64_selected_memory_form
                };
                let encoded = encode(physical, kind, alternative, &views, address.displacement)
                    .map_err(OptimizedSelectedFormEncodingError::X86_64)?;
                let footprint = encoded.footprint();
                validate_operand_footprint(
                    selected.id,
                    machine,
                    &footprint.encoded,
                    &footprint.register_reads,
                    &footprint.register_writes,
                )?;
                if footprint.encoded != machine.alternative.encoded {
                    return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
                }
                validate_size(selected.id, machine.alternative.size, encoded.bytes().len())?;
                SelectedFormEncodingState::Encoded {
                    bytes: encoded.bytes().to_vec(),
                    footprint: Box::new(SelectedFormDecodedFootprint {
                        register_reads: footprint.register_reads.clone(),
                        register_writes: footprint.register_writes.clone(),
                        implicit_defs: footprint.encoded.implicit_unit_defs.clone(),
                        implicit_clobbers: footprint.encoded.implicit_unit_clobbers.clone(),
                        encoded: footprint.encoded.clone(),
                    }),
                }
            }
        }
        kind @ (SelectedInstructionKind::CallScalar { .. }
        | SelectedInstructionKind::CallUnit { .. }
        | SelectedInstructionKind::CallAggregate { .. }) => {
            scalar_call::encode(target, selected.id, kind, machine, physical)?
        }
        SelectedInstructionKind::ConditionalBranchNonZero
        | SelectedInstructionKind::ConditionalBranchU64LessThan
        | SelectedInstructionKind::ConditionalBranchI64LessThan
        | SelectedInstructionKind::Jump => SelectedFormEncodingState::DeferredControl {
            reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
        },
        kind => encode_scalar(target, selected.id, kind, alternative, machine, physical)?,
    };
    Ok(SelectedFormEncodingRow {
        instruction: selected.id,
        alternative,
        machine_disposition: SelectedFormMachineDisposition::RetainedV1,
        state,
        address,
    })
}

fn encode_scalar(
    target: NativeTarget,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<SelectedFormEncodingState, OptimizedSelectedFormEncodingError> {
    let views = machine
        .operands
        .iter()
        .map(|operand| operand.view)
        .collect::<Vec<_>>();
    let (bytes, reads, writes, encoded_effects) = match target.architecture {
        Architecture::X86_64 => {
            let encoded = if kind == SelectedInstructionKind::HostedExitProcessI32 {
                isa_x86_64::encode_x86_64_selected_hosted_exit_process_form(
                    target,
                    physical,
                    kind,
                    alternative,
                    &views,
                )
            } else {
                encode_x86_64_selected_form(physical, kind, alternative, &views)
            }
            .map_err(OptimizedSelectedFormEncodingError::X86_64)?;
            (
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().register_writes.clone(),
                encoded.footprint().encoded.clone(),
            )
        }
        Architecture::Aarch64 => {
            let encoded = if kind == SelectedInstructionKind::HostedExitProcessI32 {
                isa_aarch64::encode_aarch64_selected_hosted_exit_process_form(
                    target,
                    physical,
                    kind,
                    alternative,
                    &views,
                )
            } else {
                encode_aarch64_selected_form(physical, kind, alternative, &views)
            }
            .map_err(OptimizedSelectedFormEncodingError::Aarch64)?;
            (
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().register_writes.clone(),
                encoded.footprint().encoded.clone(),
            )
        }
    };
    validate_operand_footprint(instruction, machine, &encoded_effects, &reads, &writes)?;
    if encoded_effects != machine.alternative.encoded {
        return Err(OptimizedSelectedFormEncodingError::ImplicitFootprintMismatch(instruction));
    }
    validate_size(instruction, machine.alternative.size, bytes.len())?;
    Ok(SelectedFormEncodingState::Encoded {
        bytes,
        footprint: Box::new(SelectedFormDecodedFootprint {
            register_reads: reads,
            register_writes: writes,
            implicit_defs: encoded_effects.implicit_unit_defs.clone(),
            implicit_clobbers: encoded_effects.implicit_unit_clobbers.clone(),
            encoded: encoded_effects,
        }),
    })
}

fn validate_operand_footprint(
    instruction: SelectedInstructionId,
    machine: &PostAllocationMachineInstruction,
    encoded: &MachineEncodedEffects,
    reads: &[RegisterViewId],
    writes: &[RegisterViewId],
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let resolve = |operand: u16| {
        machine
            .operands
            .iter()
            .find(|row| row.operand == operand)
            .map(|row| row.view)
    };
    let expected_reads = encoded
        .external_operand_reads
        .iter()
        .map(|operand| resolve(*operand))
        .collect::<Option<Vec<_>>>()
        .ok_or(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(instruction))?;
    let expected_writes = encoded
        .external_operand_writes
        .iter()
        .map(|operand| resolve(*operand))
        .collect::<Option<Vec<_>>>()
        .ok_or(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(instruction))?;
    if reads != expected_reads || writes != expected_writes {
        return Err(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(instruction));
    }
    Ok(())
}

fn validate_size(
    instruction: SelectedInstructionId,
    knowledge: MachineSizeKnowledge,
    actual: usize,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let actual = u16::try_from(actual)
        .map_err(|_| OptimizedSelectedFormEncodingError::SizeDeclarationMismatch(instruction))?;
    let matches = match knowledge {
        MachineSizeKnowledge::ExactBytes(expected) => actual == expected,
        MachineSizeKnowledge::EncoderResolved {
            minimum_bytes,
            maximum_bytes,
        } => actual >= minimum_bytes && maximum_bytes.is_none_or(|maximum| actual <= maximum),
    };
    if !matches {
        return Err(OptimizedSelectedFormEncodingError::SizeDeclarationMismatch(
            instruction,
        ));
    }
    Ok(())
}
