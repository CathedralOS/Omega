use isa_aarch64::validate_aarch64_selected_form_encoding;
use isa_x86_64::validate_x86_64_selected_form_encoding;
use physical_instructions::PostAllocationMachineInstruction;
use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineEncodedEffects, MachineSizeKnowledge, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind,
};
use target::{Architecture, NativeTarget};

use super::super::{
    DeferredControlEncodingReason, OptimizedSelectedFormEncodingError,
    SelectedFormDecodedFootprint, SelectedFormEncodingRow, SelectedFormEncodingState,
    SelectedFormMachineDisposition,
};

mod scalar_call;

pub(crate) fn validate(
    target: NativeTarget,
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    row: &SelectedFormEncodingRow,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let architecture = target.architecture;
    if row.instruction != selected.id
        || row.alternative != machine.alternative.key
        || row.machine_disposition != SelectedFormMachineDisposition::RetainedV1
    {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    match selected.kind {
        kind @ (SelectedInstructionKind::Store { .. }
        | SelectedInstructionKind::AddressOffset { .. }
        | SelectedInstructionKind::Load64 { .. }
        | SelectedInstructionKind::HostedWriteByteI32 { .. }
        | SelectedInstructionKind::Load8Indexed
        | SelectedInstructionKind::Store64 { .. }
        | SelectedInstructionKind::FrameAddress { .. }) => {
            let address = row
                .address
                .ok_or(OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
            let SelectedFormEncodingState::Encoded { bytes, footprint } = &row.state else {
                return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
            };
            let decoded = if architecture == Architecture::Aarch64 {
                let encoded = if matches!(kind, SelectedInstructionKind::HostedWriteByteI32 { .. })
                {
                    isa_aarch64::validate_aarch64_selected_hosted_write_byte_form(
                        target,
                        physical,
                        kind,
                        machine.alternative.key,
                        &operand_views(machine),
                        address.displacement,
                        bytes,
                    )
                } else {
                    isa_aarch64::validate_aarch64_selected_memory_form(
                        physical,
                        kind,
                        machine.alternative.key,
                        &operand_views(machine),
                        address.displacement,
                        bytes,
                    )
                }
                .map_err(OptimizedSelectedFormEncodingError::Aarch64)?;
                decoded_footprint(
                    &encoded.footprint().register_reads,
                    &encoded.footprint().register_writes,
                    &encoded.footprint().encoded,
                )
            } else {
                let encode = if matches!(kind, SelectedInstructionKind::HostedWriteByteI32 { .. }) {
                    isa_x86_64::validate_x86_64_selected_hosted_write_byte_form
                } else {
                    isa_x86_64::validate_x86_64_selected_memory_form
                };
                let encoded = encode(
                    physical,
                    kind,
                    machine.alternative.key,
                    &operand_views(machine),
                    address.displacement,
                    bytes,
                )
                .map_err(OptimizedSelectedFormEncodingError::X86_64)?;
                decoded_footprint(
                    &encoded.footprint().register_reads,
                    &encoded.footprint().register_writes,
                    &encoded.footprint().encoded,
                )
            };
            validate_machine_footprint(selected.id, machine, &decoded)?;
            validate_size(selected.id, machine.alternative.size, bytes.len())?;
            if footprint.as_ref() != &decoded {
                return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
            }
            Ok(())
        }
        kind @ (SelectedInstructionKind::CallI64 { .. }
        | SelectedInstructionKind::CallUnit { .. }) => {
            scalar_call::validate(target, selected.id, kind, machine, physical, &row.state)
        }
        SelectedInstructionKind::ConditionalBranchNonZero
        | SelectedInstructionKind::ConditionalBranchU64LessThan
        | SelectedInstructionKind::ConditionalBranchI64LessThan
        | SelectedInstructionKind::Jump => {
            if row.state
                != (SelectedFormEncodingState::DeferredControl {
                    reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
                })
            {
                return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
            }
            Ok(())
        }
        kind => validate_baseline(
            architecture,
            selected.id,
            kind,
            machine,
            physical,
            &row.state,
        ),
    }
}

fn validate_baseline(
    architecture: Architecture,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    state: &SelectedFormEncodingState,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let SelectedFormEncodingState::Encoded { bytes, footprint } = state else {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    };
    let views = operand_views(machine);
    let decoded = match architecture {
        Architecture::X86_64 => {
            let decoded = validate_x86_64_selected_form_encoding(
                physical,
                kind,
                machine.alternative.key,
                &views,
                bytes,
            )
            .map_err(|_| OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
            decoded_footprint(
                &decoded.footprint().register_reads,
                &decoded.footprint().register_writes,
                &decoded.footprint().encoded,
            )
        }
        Architecture::Aarch64 => {
            let decoded = validate_aarch64_selected_form_encoding(
                physical,
                kind,
                machine.alternative.key,
                &views,
                bytes,
            )
            .map_err(|_| OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
            decoded_footprint(
                &decoded.footprint().register_reads,
                &decoded.footprint().register_writes,
                &decoded.footprint().encoded,
            )
        }
    };
    validate_machine_footprint(instruction, machine, &decoded)?;
    validate_size(instruction, machine.alternative.size, bytes.len())?;
    if footprint.as_ref() != &decoded {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    Ok(())
}

fn decoded_footprint(
    reads: &[RegisterViewId],
    writes: &[RegisterViewId],
    encoded: &MachineEncodedEffects,
) -> SelectedFormDecodedFootprint {
    SelectedFormDecodedFootprint {
        register_reads: reads.to_vec(),
        register_writes: writes.to_vec(),
        implicit_defs: encoded.implicit_unit_defs.clone(),
        implicit_clobbers: encoded.implicit_unit_clobbers.clone(),
        encoded: encoded.clone(),
    }
}

fn validate_machine_footprint(
    instruction: SelectedInstructionId,
    machine: &PostAllocationMachineInstruction,
    decoded: &SelectedFormDecodedFootprint,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    validate_external_operands(instruction, machine, decoded)?;
    if decoded.encoded != machine.alternative.encoded {
        return Err(OptimizedSelectedFormEncodingError::ImplicitFootprintMismatch(instruction));
    }
    Ok(())
}

fn validate_external_operands(
    instruction: SelectedInstructionId,
    machine: &PostAllocationMachineInstruction,
    decoded: &SelectedFormDecodedFootprint,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let resolve = |operand: u16| {
        machine
            .operands
            .iter()
            .find(|row| row.operand == operand)
            .map(|row| row.view)
    };
    let expected_reads = decoded
        .encoded
        .external_operand_reads
        .iter()
        .map(|operand| resolve(*operand))
        .collect::<Option<Vec<_>>>()
        .ok_or(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(instruction))?;
    let expected_writes = decoded
        .encoded
        .external_operand_writes
        .iter()
        .map(|operand| resolve(*operand))
        .collect::<Option<Vec<_>>>()
        .ok_or(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(instruction))?;
    if decoded.register_reads != expected_reads || decoded.register_writes != expected_writes {
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

fn operand_views(machine: &PostAllocationMachineInstruction) -> Vec<RegisterViewId> {
    machine
        .operands
        .iter()
        .map(|operand| operand.view)
        .collect()
}
