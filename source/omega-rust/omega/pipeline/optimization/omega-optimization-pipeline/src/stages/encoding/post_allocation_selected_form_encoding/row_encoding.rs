use omega_isa_aarch64::encode_aarch64_selected_form;
use omega_isa_x86_64::encode_x86_64_selected_form;
use omega_machine_optimizer::PostAllocationMachineInstruction;
use omega_register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use omega_selected_instructions::{
    MachineAlternativeKey, MachineEncodedEffects, MachineSizeKnowledge, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind,
};
use omega_target::Architecture;

use super::{
    DeferredControlEncodingReason, OptimizedSelectedFormEncodingError,
    SelectedFormDecodedFootprint, SelectedFormEncodingRow, SelectedFormEncodingState,
    SelectedFormMachineDisposition, materialization::MaterializationDisposition,
};

mod aarch64_movn;
mod x86_mov_r32_imm32;
mod x86_mov_r64_imm32_sign_extended;
mod x86_xor_zero;

pub(super) fn encode_row(
    architecture: Architecture,
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    machine_disposition: SelectedFormMachineDisposition,
    materialization: Option<MaterializationDisposition<'_>>,
) -> Result<SelectedFormEncodingRow, OptimizedSelectedFormEncodingError> {
    validate_machine_disposition(
        architecture,
        selected,
        machine,
        physical,
        &machine_disposition,
    )?;
    let alternative = machine.alternative.key;
    let state = match (selected.kind, materialization) {
        (
            kind @ SelectedInstructionKind::MaterializeI64 { .. },
            Some(MaterializationDisposition::Aarch64Movn(disposition)),
        ) => aarch64_movn::encode(architecture, selected, kind, machine, physical, disposition)?,
        (
            kind @ SelectedInstructionKind::MaterializeI64 { .. },
            Some(MaterializationDisposition::X86XorZero(disposition)),
        ) => x86_xor_zero::encode(architecture, selected, kind, machine, physical, disposition)?,
        (
            kind @ SelectedInstructionKind::MaterializeI64 { .. },
            Some(MaterializationDisposition::X86MovR32Imm32(disposition)),
        ) => {
            x86_mov_r32_imm32::encode(architecture, selected, kind, machine, physical, disposition)?
        }
        (
            kind @ SelectedInstructionKind::MaterializeI64 { .. },
            Some(MaterializationDisposition::X86MovR64Imm32SignExtended(disposition)),
        ) => x86_mov_r64_imm32_sign_extended::encode(
            architecture,
            selected,
            kind,
            machine,
            physical,
            disposition,
        )?,
        (
            SelectedInstructionKind::ConditionalBranchNonZero
            | SelectedInstructionKind::ConditionalBranchU64LessThan,
            materialization,
        ) if materialization.is_none_or(MaterializationDisposition::is_retained) => {
            SelectedFormEncodingState::DeferredControl {
                reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
            }
        }
        (kind, materialization)
            if materialization.is_none_or(MaterializationDisposition::is_retained) =>
        {
            encode_scalar(
                architecture,
                selected.id,
                kind,
                alternative,
                machine,
                physical,
            )?
        }
        _ => {
            return Err(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(selected.id));
        }
    };
    Ok(SelectedFormEncodingRow {
        instruction: selected.id,
        alternative,
        machine_disposition,
        state,
    })
}

fn integer_bits(value: psi_core::IntegerValue) -> Option<u64> {
    match value {
        psi_core::IntegerValue::Signed(value) => {
            i64::try_from(value).ok().map(|value| value as u64)
        }
        psi_core::IntegerValue::Unsigned(value) => u64::try_from(value).ok(),
    }
}

fn validate_machine_disposition(
    architecture: Architecture,
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    disposition: &SelectedFormMachineDisposition,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let valid = match disposition {
        SelectedFormMachineDisposition::RetainedV1 => true,
        SelectedFormMachineDisposition::Aarch64ElidedCompareI64ZeroV1 { consumer } => {
            architecture == Architecture::Aarch64
                && matches!(selected.kind, SelectedInstructionKind::CompareI64Zero)
                && *consumer != selected.id
        }
        SelectedFormMachineDisposition::Aarch64FusedBranchNonZeroToCbnzV1 {
            compare,
            source_read,
        } => {
            let view = physical
                .model()
                .views
                .iter()
                .find(|view| view.id == source_read.view);
            architecture == Architecture::Aarch64
                && matches!(
                    selected.kind,
                    SelectedInstructionKind::ConditionalBranchNonZero
                )
                && machine.operands.is_empty()
                && *compare == source_read.source_instruction
                && *compare != selected.id
                && source_read.operand == 0
                && view.is_some_and(|view| {
                    view.class == source_read.class && view.units == source_read.units
                })
        }
        SelectedFormMachineDisposition::Aarch64ElidedSameViewCopyI64V1 { consumer } => {
            architecture == Architecture::Aarch64
                && matches!(selected.kind, SelectedInstructionKind::CopyI64)
                && *consumer != selected.id
        }
    };
    if !valid {
        return Err(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(selected.id));
    }
    Ok(())
}

fn encode_scalar(
    architecture: Architecture,
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
    let (bytes, reads, writes, encoded_effects) = match architecture {
        Architecture::X86_64 => {
            let encoded = encode_x86_64_selected_form(physical, kind, alternative, &views)
                .map_err(OptimizedSelectedFormEncodingError::X86_64)?;
            (
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().register_writes.clone(),
                encoded.footprint().encoded.clone(),
            )
        }
        Architecture::Aarch64 => {
            let encoded = encode_aarch64_selected_form(physical, kind, alternative, &views)
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

#[cfg(test)]
mod tests;
