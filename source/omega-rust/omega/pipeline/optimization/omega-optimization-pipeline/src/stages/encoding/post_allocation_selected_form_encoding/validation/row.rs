use omega_isa_aarch64::{
    aarch64_shortest_movn_materialization_recipe, validate_aarch64_selected_form_encoding,
    validate_aarch64_shortest_movn_materialization,
};
use omega_isa_x86_64::{
    validate_x86_64_selected_form_encoding, validate_x86_64_xor_zero_i64_materialization,
};
use omega_machine_optimizer::{
    Aarch64CbnzInstructionDisposition, Aarch64MovnInstructionDisposition,
    PostAllocationMachineInstruction, X86_MOVABS_I64_BYTE_COUNT, X86_XOR_R64_SELF_BYTE_COUNT,
    X86XorZeroInstructionDisposition,
};
use omega_register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use omega_selected_instructions::{
    MachineEncodedEffects, MachineSizeKnowledge, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind,
};
use omega_target::Architecture;

use super::super::{
    DeferredControlEncodingReason, OptimizedSelectedFormEncodingError,
    SelectedFormDecodedFootprint, SelectedFormEncodingRow, SelectedFormEncodingState,
};

pub(super) fn validate(
    architecture: Architecture,
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    machine_disposition: &Aarch64CbnzInstructionDisposition,
    movn_disposition: Option<&Aarch64MovnInstructionDisposition>,
    xor_zero_disposition: Option<&X86XorZeroInstructionDisposition>,
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
    match (selected.kind, movn_disposition, xor_zero_disposition) {
        (kind @ SelectedInstructionKind::MaterializeI64 { .. }, Some(disposition), None) => {
            validate_movn(
                architecture,
                selected,
                kind,
                machine,
                physical,
                disposition,
                &row.state,
            )
        }
        (kind @ SelectedInstructionKind::MaterializeI64 { .. }, None, Some(disposition)) => {
            validate_xor_zero(
                architecture,
                selected,
                kind,
                machine,
                physical,
                disposition,
                &row.state,
            )
        }
        (SelectedInstructionKind::ConditionalBranchNonZero, movn, xor_zero)
            if movn.is_none_or(|disposition| {
                matches!(disposition, Aarch64MovnInstructionDisposition::RetainedV1)
            }) && xor_zero.is_none_or(|disposition| {
                matches!(disposition, X86XorZeroInstructionDisposition::RetainedV1)
            }) =>
        {
            if row.state
                != (SelectedFormEncodingState::DeferredControl {
                    reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
                })
            {
                return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
            }
            Ok(())
        }
        (kind, Some(Aarch64MovnInstructionDisposition::RetainedV1), None)
        | (kind, None, Some(X86XorZeroInstructionDisposition::RetainedV1))
        | (kind, None, None) => validate_baseline(
            architecture,
            selected.id,
            kind,
            machine,
            physical,
            &row.state,
        ),
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

fn validate_movn(
    architecture: Architecture,
    selected: &SelectedInstruction,
    kind: SelectedInstructionKind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    disposition: &Aarch64MovnInstructionDisposition,
    state: &SelectedFormEncodingState,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let Aarch64MovnInstructionDisposition::MovnSeededMaterializationV1 {
        literal_bits,
        destination,
        baseline_word_count,
        recipe,
    } = disposition
    else {
        return validate_baseline(architecture, selected.id, kind, machine, physical, state);
    };
    let SelectedInstructionKind::MaterializeI64 { value } = kind else {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    };
    let operand = machine
        .operands
        .first()
        .filter(|_| machine.operands.len() == 1);
    let destination_matches = operand.is_some_and(|operand| {
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
    let expected_recipe = aarch64_shortest_movn_materialization_recipe(value)
        .map_err(|_| OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
    let recipe_matches = usize::from(*baseline_word_count) * 4
        == expected_recipe.baseline_byte_count()
        && recipe.seed_halfword == expected_recipe.seed().halfword()
        && recipe.seed_immediate == expected_recipe.seed().immediate()
        && recipe.patches.len() == expected_recipe.patches().len()
        && recipe
            .patches
            .iter()
            .zip(expected_recipe.patches())
            .all(|(left, right)| {
                left.halfword == right.halfword() && left.immediate == right.immediate()
            });
    if !destination_matches || integer_bits(value) != Some(*literal_bits) || !recipe_matches {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    let SelectedFormEncodingState::Encoded { bytes, footprint } = state else {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    };
    let decoded =
        validate_aarch64_shortest_movn_materialization(physical, destination.view, value, bytes)
            .map_err(|_| OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
    let decoded = decoded_footprint(
        &decoded.footprint().register_reads,
        &decoded.footprint().register_writes,
        &decoded.footprint().encoded,
    );
    validate_machine_footprint(selected.id, machine, &decoded)?;
    validate_size(selected.id, machine.alternative.size, bytes.len())?;
    if footprint.as_ref() != &decoded {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    Ok(())
}

fn validate_xor_zero(
    architecture: Architecture,
    selected: &SelectedInstruction,
    kind: SelectedInstructionKind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    disposition: &X86XorZeroInstructionDisposition,
    state: &SelectedFormEncodingState,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let X86XorZeroInstructionDisposition::XorZeroMaterializationV1 {
        destination,
        rflags_units,
        baseline_byte_count,
        selected_byte_count,
    } = disposition
    else {
        return validate_baseline(architecture, selected.id, kind, machine, physical, state);
    };
    let SelectedInstructionKind::MaterializeI64 { value } = kind else {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
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
    if architecture != Architecture::X86_64
        || !destination_matches
        || integer_bits(value) != Some(0)
        || *baseline_byte_count != X86_MOVABS_I64_BYTE_COUNT
        || *selected_byte_count != X86_XOR_R64_SELF_BYTE_COUNT
    {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    let SelectedFormEncodingState::Encoded { bytes, footprint } = state else {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    };
    let decoded = validate_x86_64_xor_zero_i64_materialization(physical, destination.view, bytes)
        .map_err(|_| OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
    let decoded_footprint = decoded_footprint(
        &decoded.footprint().register_reads,
        &decoded.footprint().register_writes,
        &decoded.footprint().encoded,
    );
    validate_external_operands(selected.id, machine, &decoded_footprint)?;
    if bytes.len() != usize::from(X86_XOR_R64_SELF_BYTE_COUNT)
        || decoded.value_bits() != 0
        || decoded.destination() != destination.view
        || !decoded.footprint().writes_rflags
        || !decoded.footprint().register_reads.is_empty()
        || decoded.footprint().register_writes.as_slice() != [destination.view]
        || !decoded.footprint().encoded.implicit_unit_uses.is_empty()
        || !decoded.footprint().encoded.implicit_unit_defs.is_empty()
        || decoded.footprint().encoded.implicit_unit_clobbers != *rflags_units
        || footprint.as_ref() != &decoded_footprint
    {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    Ok(())
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
