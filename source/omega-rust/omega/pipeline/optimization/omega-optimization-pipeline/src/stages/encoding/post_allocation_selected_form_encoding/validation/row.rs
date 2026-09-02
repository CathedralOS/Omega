use omega_isa_aarch64::validate_aarch64_selected_form_encoding;
use omega_isa_x86_64::validate_x86_64_selected_form_encoding;
use omega_machine_optimizer::PostAllocationMachineInstruction;
use omega_register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use omega_selected_instructions::{
    MachineEncodedEffects, MachineSizeKnowledge, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind,
};
use omega_target::Architecture;

use super::super::{
    DeferredControlEncodingReason, OptimizedSelectedFormEncodingError,
    SelectedFormDecodedFootprint, SelectedFormEncodingRow, SelectedFormEncodingState,
    SelectedFormMachineDisposition, materialization::MaterializationDisposition,
};

mod aarch64_movn;
mod x86_mov_r32_imm32;
mod x86_mov_r64_imm32_sign_extended;
mod x86_xor_zero;

pub(super) fn validate(
    architecture: Architecture,
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    machine_disposition: &SelectedFormMachineDisposition,
    materialization: Option<MaterializationDisposition<'_>>,
    row: &SelectedFormEncodingRow,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    validate_machine_disposition(
        architecture,
        selected,
        machine,
        physical,
        machine_disposition,
    )?;
    if row.instruction != selected.id
        || row.alternative != machine.alternative.key
        || &row.machine_disposition != machine_disposition
    {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    match (selected.kind, materialization) {
        (
            kind @ SelectedInstructionKind::MaterializeI64 { .. },
            Some(MaterializationDisposition::Aarch64Movn(disposition)),
        ) => aarch64_movn::validate(
            architecture,
            selected,
            kind,
            machine,
            physical,
            disposition,
            &row.state,
        ),
        (
            kind @ SelectedInstructionKind::MaterializeI64 { .. },
            Some(MaterializationDisposition::X86XorZero(disposition)),
        ) => x86_xor_zero::validate(
            architecture,
            selected,
            kind,
            machine,
            physical,
            disposition,
            &row.state,
        ),
        (
            kind @ SelectedInstructionKind::MaterializeI64 { .. },
            Some(MaterializationDisposition::X86MovR32Imm32(disposition)),
        ) => x86_mov_r32_imm32::validate(
            architecture,
            selected,
            kind,
            machine,
            physical,
            disposition,
            &row.state,
        ),
        (
            kind @ SelectedInstructionKind::MaterializeI64 { .. },
            Some(MaterializationDisposition::X86MovR64Imm32SignExtended(disposition)),
        ) => x86_mov_r64_imm32_sign_extended::validate(
            architecture,
            selected,
            kind,
            machine,
            physical,
            disposition,
            &row.state,
        ),
        (
            SelectedInstructionKind::ConditionalBranchNonZero
            | SelectedInstructionKind::ConditionalBranchU64LessThan,
            materialization,
        ) if materialization.is_none_or(MaterializationDisposition::is_retained) => {
            if row.state
                != (SelectedFormEncodingState::DeferredControl {
                    reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
                })
            {
                return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
            }
            Ok(())
        }
        (kind, materialization)
            if materialization.is_none_or(MaterializationDisposition::is_retained) =>
        {
            validate_baseline(
                architecture,
                selected.id,
                kind,
                machine,
                physical,
                &row.state,
            )
        }
        _ => Err(OptimizedSelectedFormEncodingError::ArtifactMismatch),
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

fn integer_bits(value: psi_core::IntegerValue) -> Option<u64> {
    match value {
        psi_core::IntegerValue::Signed(value) => {
            i64::try_from(value).ok().map(|value| value as u64)
        }
        psi_core::IntegerValue::Unsigned(value) => u64::try_from(value).ok(),
    }
}
