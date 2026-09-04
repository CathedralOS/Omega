//! Independent validator for AArch64 shortest MOVN-seeded rows.

use omega_isa_aarch64::{
    aarch64_shortest_movn_materialization_recipe, validate_aarch64_shortest_movn_materialization,
};
use omega_machine_optimizer::{
    Aarch64MovnInstructionDisposition, PostAllocationMachineInstruction,
};
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions::{SelectedInstruction, SelectedInstructionKind};
use omega_target::Architecture;

use super::{
    decoded_footprint, integer_bits, validate_baseline, validate_machine_footprint, validate_size,
};
use crate::{OptimizedSelectedFormEncodingError, SelectedFormEncodingState};

pub(super) fn validate(
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
