//! Independent V2 replay comparison and receipt sealing.

use crate::{
    HomedSpillPseudoInstruction, HomedSpillPseudoInstructionError, HomedSpillPseudoInstructionPlan,
    HomedSpillPseudoInstructionReceipt, ValidatedHomedSpillPseudoInstructions,
    ValidatedRecursiveReloadValueHomes, ValidatedSpillPseudoInstructions,
    homed_spill_pseudo_instruction_plan_identity,
};

pub fn validate_homed_spill_pseudo_instructions(
    source: &ValidatedSpillPseudoInstructions,
    homes: &ValidatedRecursiveReloadValueHomes,
    candidate: HomedSpillPseudoInstructionPlan,
) -> Result<ValidatedHomedSpillPseudoInstructions, HomedSpillPseudoInstructionError> {
    let source_receipt = source.receipt();
    let home_receipt = homes.receipt();
    if candidate.spill_pseudo_instructions != source_receipt.identity()
        || candidate.recursive_reload_value_homes != home_receipt.identity()
        || candidate.register_environment != source_receipt.register_environment()
        || candidate.allocator_availability != source_receipt.allocator_availability()
        || candidate.optimization_unit != source_receipt.optimization_unit()
        || candidate.fuel_schedule != source_receipt.fuel_schedule()
    {
        return Err(HomedSpillPseudoInstructionError::RootMismatch);
    }
    let expected = super::replay::replay(source, homes, candidate.policy, candidate.budget)?;
    if candidate.usage != expected.usage {
        return Err(HomedSpillPseudoInstructionError::UsageMismatch);
    }
    if candidate.functions != expected.functions {
        return Err(HomedSpillPseudoInstructionError::NonCanonicalFunctions);
    }
    let storage_count = candidate
        .functions
        .iter()
        .map(|row| row.storage.len())
        .sum();
    let instruction_count = candidate
        .functions
        .iter()
        .map(|row| row.instructions.len())
        .sum();
    let reload_count = candidate
        .functions
        .iter()
        .flat_map(|row| &row.instructions)
        .filter(|instruction| matches!(instruction, HomedSpillPseudoInstruction::Reload { .. }))
        .count();
    let rewrite_count = candidate
        .functions
        .iter()
        .map(|row| row.rewrites.len())
        .sum();
    let max_spill_area_bytes = candidate
        .functions
        .iter()
        .map(|row| row.spill_area_bytes)
        .max()
        .unwrap_or(0);
    let receipt = HomedSpillPseudoInstructionReceipt {
        identity: homed_spill_pseudo_instruction_plan_identity(&candidate),
        spill_pseudo_instructions: candidate.spill_pseudo_instructions,
        recursive_reload_value_homes: candidate.recursive_reload_value_homes,
        register_environment: candidate.register_environment,
        allocator_availability: candidate.allocator_availability,
        optimization_unit: candidate.optimization_unit,
        fuel_schedule: candidate.fuel_schedule,
        usage: candidate.usage,
        function_count: candidate.functions.len(),
        storage_count,
        instruction_count,
        reload_count,
        rewrite_count,
        max_spill_area_bytes,
    };
    Ok(ValidatedHomedSpillPseudoInstructions {
        plan: candidate,
        receipt,
    })
}
