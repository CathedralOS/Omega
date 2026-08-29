use omega_isa_aarch64::{
    aarch64_shortest_movn_materialization_recipe, encode_aarch64_selected_form,
    encode_aarch64_shortest_movn_materialization,
};
use omega_isa_x86_64::{
    encode_x86_64_mov_r32_imm32_i64_materialization, encode_x86_64_selected_form,
    encode_x86_64_xor_zero_i64_materialization,
};
use omega_machine_optimizer::{
    Aarch64CbnzInstructionDisposition, Aarch64MovnInstructionDisposition,
    PostAllocationMachineInstruction, X86_MOV_R32_IMM32_BASELINE_BYTE_COUNT,
    X86_MOVABS_I64_BYTE_COUNT, X86_XOR_R64_SELF_BYTE_COUNT, X86MovR32Imm32InstructionDisposition,
    X86XorZeroInstructionDisposition,
};
use omega_register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use omega_selected_instructions::{
    MachineAlternativeKey, MachineEncodedEffects, MachineSizeKnowledge, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind,
};
use omega_target::Architecture;

use super::{
    DeferredControlEncodingReason, OptimizedSelectedFormEncodingError,
    SelectedFormDecodedFootprint, SelectedFormEncodingRow, SelectedFormEncodingState,
    materialization::MaterializationDisposition,
};

pub(super) fn encode_row(
    architecture: Architecture,
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    machine_disposition: Aarch64CbnzInstructionDisposition,
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
        ) => encode_aarch64_movn_row(architecture, selected, kind, machine, physical, disposition)?,
        (
            kind @ SelectedInstructionKind::MaterializeI64 { .. },
            Some(MaterializationDisposition::X86XorZero(disposition)),
        ) => encode_x86_xor_zero_row(architecture, selected, kind, machine, physical, disposition)?,
        (
            kind @ SelectedInstructionKind::MaterializeI64 { .. },
            Some(MaterializationDisposition::X86MovR32Imm32(disposition)),
        ) => encode_x86_mov_r32_imm32_row(
            architecture,
            selected,
            kind,
            machine,
            physical,
            disposition,
        )?,
        (SelectedInstructionKind::ConditionalBranchNonZero, materialization)
            if materialization.is_none_or(MaterializationDisposition::is_retained) =>
        {
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

fn encode_x86_mov_r32_imm32_row(
    architecture: Architecture,
    selected: &SelectedInstruction,
    kind: SelectedInstructionKind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    disposition: &X86MovR32Imm32InstructionDisposition,
) -> Result<SelectedFormEncodingState, OptimizedSelectedFormEncodingError> {
    let X86MovR32Imm32InstructionDisposition::MovR32Imm32MaterializationV1 {
        literal_bits,
        destination,
        baseline_byte_count,
        selected_byte_count,
        ..
    } = disposition
    else {
        return encode_scalar(
            architecture,
            selected.id,
            kind,
            machine.alternative.key,
            machine,
            physical,
        );
    };
    let SelectedInstructionKind::MaterializeI64 { value } = kind else {
        return Err(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(selected.id));
    };
    let operand = machine
        .operands
        .first()
        .filter(|_| machine.operands.len() == 1);
    let destination_matches = operand.is_some_and(|operand| {
        architecture == Architecture::X86_64
            && destination.instruction == selected.id
            && destination.operand == operand.operand
            && destination.virtual_register == operand.virtual_register
            && destination.class == operand.class
            && destination.destination_view == operand.view
            && destination.destination_storage_units == operand.storage_units
            && destination.destination_write_units == operand.write_units
            && Some(destination.destination_write_semantics) == operand.write_semantics
    });
    if !destination_matches
        || integer_bits(value) != Some(*literal_bits)
        || *baseline_byte_count != X86_MOV_R32_IMM32_BASELINE_BYTE_COUNT
    {
        return Err(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(selected.id));
    }
    let encoded = encode_x86_64_mov_r32_imm32_i64_materialization(
        physical,
        destination.destination_view,
        value,
    )
    .map_err(OptimizedSelectedFormEncodingError::X86_64MovR32Imm32)?;
    let footprint = encoded.footprint();
    validate_operand_footprint(
        selected.id,
        machine,
        &footprint.encoded,
        &footprint.register_reads,
        &footprint.register_writes,
    )?;
    if encoded.bytes().len() != usize::from(*selected_byte_count)
        || encoded.destination() != destination.destination_view
        || encoded.encoded_write_view() != destination.encoded_view
        || encoded.value_bits() != *literal_bits
        || footprint.writes_rflags
        || footprint.encoded_write_view != destination.encoded_view
        || footprint.encoded_write_view_units != destination.encoded_storage_units
        || footprint.encoded_write_units != destination.encoded_write_units
        || footprint.encoded_write_semantics != destination.encoded_write_semantics
        || footprint.encoded != machine.alternative.encoded
    {
        return Err(OptimizedSelectedFormEncodingError::ImplicitFootprintMismatch(selected.id));
    }
    Ok(SelectedFormEncodingState::Encoded {
        bytes: encoded.bytes().to_vec(),
        footprint: Box::new(SelectedFormDecodedFootprint {
            register_reads: footprint.register_reads.clone(),
            register_writes: footprint.register_writes.clone(),
            implicit_defs: footprint.encoded.implicit_unit_defs.clone(),
            implicit_clobbers: footprint.encoded.implicit_unit_clobbers.clone(),
            encoded: footprint.encoded.clone(),
        }),
    })
}

fn encode_x86_xor_zero_row(
    architecture: Architecture,
    selected: &SelectedInstruction,
    kind: SelectedInstructionKind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    disposition: &X86XorZeroInstructionDisposition,
) -> Result<SelectedFormEncodingState, OptimizedSelectedFormEncodingError> {
    let baseline = encode_x86_64_selected_form(
        physical,
        kind,
        machine.alternative.key,
        &machine
            .operands
            .iter()
            .map(|operand| operand.view)
            .collect::<Vec<_>>(),
    )
    .map_err(OptimizedSelectedFormEncodingError::X86_64)?;
    validate_operand_footprint(
        selected.id,
        machine,
        &baseline.footprint().encoded,
        &baseline.footprint().register_reads,
        &baseline.footprint().register_writes,
    )?;
    if architecture != Architecture::X86_64
        || baseline.bytes().len() != usize::from(X86_MOVABS_I64_BYTE_COUNT)
        || baseline.footprint().encoded != machine.alternative.encoded
    {
        return Err(OptimizedSelectedFormEncodingError::ImplicitFootprintMismatch(selected.id));
    }
    validate_size(
        selected.id,
        machine.alternative.size,
        baseline.bytes().len(),
    )?;

    let X86XorZeroInstructionDisposition::XorZeroMaterializationV1 {
        destination,
        rflags_units,
        baseline_byte_count,
        selected_byte_count,
    } = disposition
    else {
        return Ok(SelectedFormEncodingState::Encoded {
            bytes: baseline.bytes().to_vec(),
            footprint: Box::new(SelectedFormDecodedFootprint {
                register_reads: baseline.footprint().register_reads.clone(),
                register_writes: baseline.footprint().register_writes.clone(),
                implicit_defs: baseline.footprint().encoded.implicit_unit_defs.clone(),
                implicit_clobbers: baseline.footprint().encoded.implicit_unit_clobbers.clone(),
                encoded: baseline.footprint().encoded.clone(),
            }),
        });
    };
    let SelectedInstructionKind::MaterializeI64 { value } = kind else {
        return Err(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(selected.id));
    };
    let operand = machine
        .operands
        .first()
        .filter(|_| machine.operands.len() == 1);
    let destination_matches = operand.is_some_and(|operand| {
        destination.instruction == selected.id
            && destination.operand == operand.operand
            && destination.virtual_register == operand.virtual_register
            && destination.class == operand.class
            && destination.view == operand.view
            && destination.storage_units == operand.storage_units
            && destination.write_units == operand.write_units
            && Some(destination.write_semantics) == operand.write_semantics
    });
    if !destination_matches
        || integer_bits(value) != Some(0)
        || *baseline_byte_count != X86_MOVABS_I64_BYTE_COUNT
        || *selected_byte_count != X86_XOR_R64_SELF_BYTE_COUNT
    {
        return Err(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(selected.id));
    }
    let encoded = encode_x86_64_xor_zero_i64_materialization(physical, destination.view)
        .map_err(OptimizedSelectedFormEncodingError::X86_64)?;
    let footprint = encoded.footprint();
    validate_operand_footprint(
        selected.id,
        machine,
        &footprint.encoded,
        &footprint.register_reads,
        &footprint.register_writes,
    )?;
    if encoded.bytes().len() != usize::from(X86_XOR_R64_SELF_BYTE_COUNT)
        || encoded.value_bits() != 0
        || encoded.destination() != destination.view
        || !footprint.writes_rflags
        || footprint.register_reads.len() != 0
        || footprint.register_writes.as_slice() != [destination.view]
        || footprint.encoded.implicit_unit_uses.len() != 0
        || footprint.encoded.implicit_unit_defs.len() != 0
        || footprint.encoded.implicit_unit_clobbers != *rflags_units
    {
        return Err(OptimizedSelectedFormEncodingError::ImplicitFootprintMismatch(selected.id));
    }
    Ok(SelectedFormEncodingState::Encoded {
        bytes: encoded.bytes().to_vec(),
        footprint: Box::new(SelectedFormDecodedFootprint {
            register_reads: footprint.register_reads.clone(),
            register_writes: footprint.register_writes.clone(),
            implicit_defs: footprint.encoded.implicit_unit_defs.clone(),
            implicit_clobbers: footprint.encoded.implicit_unit_clobbers.clone(),
            encoded: footprint.encoded.clone(),
        }),
    })
}

fn encode_aarch64_movn_row(
    architecture: Architecture,
    selected: &SelectedInstruction,
    kind: SelectedInstructionKind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    disposition: &Aarch64MovnInstructionDisposition,
) -> Result<SelectedFormEncodingState, OptimizedSelectedFormEncodingError> {
    let Aarch64MovnInstructionDisposition::MovnSeededMaterializationV1 {
        literal_bits,
        destination,
        baseline_word_count,
        recipe,
    } = disposition
    else {
        return encode_scalar(
            architecture,
            selected.id,
            kind,
            machine.alternative.key,
            machine,
            physical,
        );
    };
    let SelectedInstructionKind::MaterializeI64 { value } = kind else {
        return Err(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(selected.id));
    };
    let operand = machine
        .operands
        .first()
        .filter(|_| machine.operands.len() == 1);
    let valid_destination = operand.is_some_and(|operand| {
        architecture == Architecture::Aarch64
            && destination.instruction == selected.id
            && destination.operand == operand.operand
            && destination.virtual_register == operand.virtual_register
            && destination.class == operand.class
            && destination.view == operand.view
            && destination.storage_units == operand.storage_units
            && destination.write_units == operand.write_units
            && Some(destination.write_semantics) == operand.write_semantics
    });
    if !valid_destination || integer_bits(value) != Some(*literal_bits) {
        return Err(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(selected.id));
    }
    let isa_recipe = aarch64_shortest_movn_materialization_recipe(value)
        .map_err(OptimizedSelectedFormEncodingError::Aarch64)?;
    let recipe_matches = usize::from(*baseline_word_count) * 4 == isa_recipe.baseline_byte_count()
        && recipe.seed_halfword == isa_recipe.seed().halfword()
        && recipe.seed_immediate == isa_recipe.seed().immediate()
        && recipe.patches.len() == isa_recipe.patches().len()
        && recipe
            .patches
            .iter()
            .zip(isa_recipe.patches())
            .all(|(left, right)| {
                left.halfword == right.halfword() && left.immediate == right.immediate()
            });
    if !recipe_matches {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    let encoded = encode_aarch64_shortest_movn_materialization(physical, destination.view, value)
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
        return Err(OptimizedSelectedFormEncodingError::ImplicitFootprintMismatch(selected.id));
    }
    validate_size(selected.id, machine.alternative.size, encoded.bytes().len())?;
    Ok(SelectedFormEncodingState::Encoded {
        bytes: encoded.bytes().to_vec(),
        footprint: Box::new(SelectedFormDecodedFootprint {
            register_reads: footprint.register_reads.clone(),
            register_writes: footprint.register_writes.clone(),
            implicit_defs: footprint.encoded.implicit_unit_defs.clone(),
            implicit_clobbers: footprint.encoded.implicit_unit_clobbers.clone(),
            encoded: footprint.encoded.clone(),
        }),
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
    disposition: &Aarch64CbnzInstructionDisposition,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let valid = match disposition {
        Aarch64CbnzInstructionDisposition::RetainedV1 => true,
        Aarch64CbnzInstructionDisposition::ElidedCompareI64ZeroV1 { consumer } => {
            architecture == Architecture::Aarch64
                && matches!(selected.kind, SelectedInstructionKind::CompareI64Zero)
                && *consumer != selected.id
        }
        Aarch64CbnzInstructionDisposition::FusedBranchNonZeroToCbnzV1 {
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
