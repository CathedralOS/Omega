//! Exact row encoder for AArch64 shortest MOVN-seeded materialization.

use isa_aarch64::{
    aarch64_shortest_movn_materialization_recipe, encode_aarch64_shortest_movn_materialization,
};
use physical_instructions::PostAllocationMachineInstruction;
use post_allocation_machine_to_post_allocation_machine::Aarch64MovnInstructionDisposition;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{SelectedInstruction, SelectedInstructionKind};
use target::Architecture;

use super::{encode_scalar, integer_bits, validate_operand_footprint, validate_size};
use crate::selected_form_encoding::{
    OptimizedSelectedFormEncodingError, SelectedFormDecodedFootprint, SelectedFormEncodingState,
};

pub(super) fn encode(
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
