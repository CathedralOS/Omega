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

mod normalized_foreign;
mod route;
mod scalar_call;

#[cfg(test)]
mod narrow_load_tests;
#[cfg(test)]
mod tests;

use route::{HostedChannel, RowRoute};

pub(super) fn encode_row(
    target: NativeTarget,
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    address: Option<machine_code::ResolvedPhysicalAddress>,
) -> Result<SelectedFormEncodingRow, OptimizedSelectedFormEncodingError> {
    let alternative = machine.alternative.key;
    let route = route::route_of(selected.kind);
    let state = match route {
        RowRoute::ResolvedAddress { operation, channel } => {
            // The declared route binds both the address's presence and its
            // symbolic operation family: a missing address, or one naming a
            // family the kind does not declare, is malformed input rather
            // than an encoding choice.
            let address = address.ok_or(OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
            if !operation.admits(address.symbolic) {
                return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
            }
            encode_address_routed(
                target,
                selected.id,
                selected.kind,
                channel,
                alternative,
                machine,
                physical,
                address,
            )?
        }
        RowRoute::InternalCallTemplate => {
            reject_unrouted_address(address)?;
            scalar_call::encode(target, selected.id, selected.kind, machine, physical)?
        }
        RowRoute::NormalizedForeignCallTemplate => {
            reject_unrouted_address(address)?;
            normalized_foreign::encode(target, selected.id, selected.kind, machine, physical)?
        }
        RowRoute::DeferredControlFlow => {
            reject_unrouted_address(address)?;
            SelectedFormEncodingState::DeferredControl {
                reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
            }
        }
        RowRoute::Ordinary { channel } => {
            reject_unrouted_address(address)?;
            encode_scalar(
                target,
                selected.id,
                selected.kind,
                channel,
                alternative,
                machine,
                physical,
            )?
        }
    };
    Ok(SelectedFormEncodingRow {
        instruction: selected.id,
        alternative,
        machine_disposition: SelectedFormMachineDisposition::RetainedV1,
        state,
        address,
    })
}

/// Routes declaring no address operation reject a resolved address on the
/// machine row: presence would attach frame geometry to a row whose selected
/// kind never asked for one.
fn reject_unrouted_address(
    address: Option<machine_code::ResolvedPhysicalAddress>,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    if address.is_some() {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn encode_address_routed(
    target: NativeTarget,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    channel: HostedChannel,
    alternative: MachineAlternativeKey,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    address: machine_code::ResolvedPhysicalAddress,
) -> Result<SelectedFormEncodingState, OptimizedSelectedFormEncodingError> {
    let views = machine
        .operands
        .iter()
        .map(|operand| operand.view)
        .collect::<Vec<_>>();
    let displacement = address.displacement;
    let (bytes, reads, writes, encoded_effects) = match target.architecture {
        Architecture::X86_64 => {
            let encoded = match channel {
                HostedChannel::None => isa_x86_64::encode_x86_64_selected_memory_form(
                    physical,
                    kind,
                    alternative,
                    &views,
                    displacement,
                ),
                HostedChannel::WriteByteI32 => {
                    isa_x86_64::encode_x86_64_selected_hosted_write_byte_form(
                        physical,
                        kind,
                        alternative,
                        &views,
                        displacement,
                    )
                }
                HostedChannel::ReadByte => {
                    isa_x86_64::encode_x86_64_selected_hosted_read_byte_form(
                        physical,
                        kind,
                        alternative,
                        &views,
                        displacement,
                    )
                }
                // The exit channel is declared only on ordinary routes.
                HostedChannel::ExitProcessI32 => {
                    return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
                }
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
            let encoded = match channel {
                HostedChannel::None => isa_aarch64::encode_aarch64_selected_memory_form(
                    physical,
                    kind,
                    alternative,
                    &views,
                    displacement,
                ),
                HostedChannel::WriteByteI32 => {
                    isa_aarch64::encode_aarch64_selected_hosted_write_byte_form(
                        target,
                        physical,
                        kind,
                        alternative,
                        &views,
                        displacement,
                    )
                }
                HostedChannel::ReadByte => {
                    isa_aarch64::encode_aarch64_selected_hosted_read_byte_form(
                        target,
                        physical,
                        kind,
                        alternative,
                        &views,
                        displacement,
                    )
                }
                // The exit channel is declared only on ordinary routes.
                HostedChannel::ExitProcessI32 => {
                    return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
                }
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
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
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

#[allow(clippy::too_many_arguments)]
fn encode_scalar(
    target: NativeTarget,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    channel: HostedChannel,
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
            let encoded = match channel {
                HostedChannel::ExitProcessI32 => {
                    isa_x86_64::encode_x86_64_selected_hosted_exit_process_form(
                        target,
                        physical,
                        kind,
                        alternative,
                        &views,
                    )
                }
                HostedChannel::None => {
                    encode_x86_64_selected_form(physical, kind, alternative, &views)
                }
                // Byte channels are declared only on resolved-address routes.
                HostedChannel::WriteByteI32 | HostedChannel::ReadByte => {
                    return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
                }
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
            let encoded = match channel {
                HostedChannel::ExitProcessI32 => {
                    isa_aarch64::encode_aarch64_selected_hosted_exit_process_form(
                        target,
                        physical,
                        kind,
                        alternative,
                        &views,
                    )
                }
                HostedChannel::None => {
                    encode_aarch64_selected_form(physical, kind, alternative, &views)
                }
                // Byte channels are declared only on resolved-address routes.
                HostedChannel::WriteByteI32 | HostedChannel::ReadByte => {
                    return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
                }
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
