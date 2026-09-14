//! Independent spill-pseudo replay and receipt admission.

use crate::{
    SpillPseudoInstructionError, SpillPseudoInstructionPlan, SpillPseudoInstructionReceipt,
    ValidatedRecursiveSpillInsertion, ValidatedSpillPseudoInstructions,
    spill_pseudo_instruction_plan_identity,
};

pub fn validate_spill_pseudo_instructions(
    source: &ValidatedRecursiveSpillInsertion,
    candidate: SpillPseudoInstructionPlan,
) -> Result<ValidatedSpillPseudoInstructions, SpillPseudoInstructionError> {
    let source_receipt = source.receipt();
    if candidate.recursive_spill_insertion != source_receipt.identity()
        || candidate.register_environment != source_receipt.register_environment()
        || candidate.allocator_availability != source_receipt.allocator_availability()
        || candidate.optimization_unit != source_receipt.optimization_unit()
        || candidate.fuel_schedule != source_receipt.fuel_schedule()
    {
        return Err(SpillPseudoInstructionError::RootMismatch);
    }
    let expected = super::replay::replay(source, candidate.policy, candidate.budget)?;
    if candidate.usage != expected.usage {
        return Err(SpillPseudoInstructionError::UsageMismatch);
    }
    if candidate.functions != expected.functions {
        return Err(SpillPseudoInstructionError::NonCanonicalFunctions);
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
    let receipt = SpillPseudoInstructionReceipt {
        identity: spill_pseudo_instruction_plan_identity(&candidate),
        recursive_spill_insertion: candidate.recursive_spill_insertion,
        register_environment: candidate.register_environment,
        allocator_availability: candidate.allocator_availability,
        optimization_unit: candidate.optimization_unit,
        fuel_schedule: candidate.fuel_schedule,
        usage: candidate.usage,
        function_count: candidate.functions.len(),
        storage_count,
        instruction_count,
        rewrite_count,
        max_spill_area_bytes,
    };
    Ok(ValidatedSpillPseudoInstructions {
        plan: candidate,
        receipt,
    })
}
