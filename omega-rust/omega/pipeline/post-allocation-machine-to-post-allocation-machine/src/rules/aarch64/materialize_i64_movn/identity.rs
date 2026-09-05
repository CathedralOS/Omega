use sha2::{Digest, Sha256};

use crate::{
    Aarch64MovnInstructionDisposition, Aarch64MovnMaterializationPlan,
    Aarch64MovnMaterializationPolicy, Aarch64MovnMaterializationRevisionIdentity,
    Aarch64MovnRecipe, QualifiedPhysicalWrite,
};
use physical_instructions::Aarch64MovnMaterializationIdentity;

const IDENTITY_DOMAIN: &[u8] = b"omega.aarch64-movn-seeded-i64-materialization.v1\0";
const REVISION_DOMAIN: &[u8] = b"omega.aarch64-movn-seeded-i64-materialization-revision.v1\0";

pub fn aarch64_movn_materialization_identity(
    plan: &Aarch64MovnMaterializationPlan,
) -> Aarch64MovnMaterializationIdentity {
    let mut hasher = Sha256::new();
    hasher.update(IDENTITY_DOMAIN);
    hasher.update(encode_content(plan));
    Aarch64MovnMaterializationIdentity::from_bytes(hasher.finalize().into())
}

pub(crate) fn revision_identity(
    source: physical_instructions::PostAllocationMachineIdentity,
    selected: selected_instructions::SelectedInstructionPlanIdentity,
    target: target::NativeTarget,
    physical: register_model::PhysicalRegisterModelIdentity,
    functions: &[crate::Aarch64MovnMaterializationFunction],
) -> Aarch64MovnMaterializationRevisionIdentity {
    let mut hasher = Sha256::new();
    hasher.update(REVISION_DOMAIN);
    hasher.update(source.bytes());
    hasher.update(selected.bytes());
    let mut bytes = Vec::new();
    encode_target(&mut bytes, target);
    hasher.update(bytes);
    hasher.update(physical.bytes());
    let mut roster = Vec::new();
    encode_functions(&mut roster, functions);
    hasher.update(roster);
    Aarch64MovnMaterializationRevisionIdentity::from_bytes(hasher.finalize().into())
}

pub(crate) fn encode_content(plan: &Aarch64MovnMaterializationPlan) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.source.bytes());
    bytes.extend_from_slice(&plan.selected.bytes());
    encode_target(&mut bytes, plan.target);
    bytes.extend_from_slice(&plan.physical_register_model.bytes());
    bytes.push(match plan.policy {
        Aarch64MovnMaterializationPolicy::Aarch64SelectShortestMovnSeededI64MaterializationV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    bytes.extend_from_slice(&plan.output_revision.bytes());
    encode_len(&mut bytes, plan.attempts.len());
    for attempt in &plan.attempts {
        bytes.extend_from_slice(&attempt.iteration.to_le_bytes());
        bytes.extend_from_slice(&attempt.input.bytes());
        bytes.extend_from_slice(&attempt.machine.get().to_le_bytes());
        bytes.extend_from_slice(&attempt.block.0.to_le_bytes());
        bytes.extend_from_slice(&attempt.instruction.0.to_le_bytes());
        bytes.extend_from_slice(&attempt.literal_bits.to_le_bytes());
        encode_write(&mut bytes, &attempt.destination);
        bytes.push(attempt.baseline_word_count);
        encode_recipe(&mut bytes, &attempt.recipe);
        bytes.push(match attempt.outcome {
            crate::Aarch64MovnMaterializationAttemptOutcome::AlreadySelected => 0,
            crate::Aarch64MovnMaterializationAttemptOutcome::BaselineNotLonger => 1,
            crate::Aarch64MovnMaterializationAttemptOutcome::SelectedForRewrite => 2,
        });
    }
    encode_len(&mut bytes, plan.actions.len());
    for action in &plan.actions {
        bytes.extend_from_slice(&action.iteration.to_le_bytes());
        bytes.extend_from_slice(&action.input.bytes());
        bytes.extend_from_slice(&action.output.bytes());
        bytes.extend_from_slice(&action.machine.get().to_le_bytes());
        bytes.extend_from_slice(&action.block.0.to_le_bytes());
        bytes.extend_from_slice(&action.instruction.0.to_le_bytes());
        bytes.extend_from_slice(&action.literal_bits.to_le_bytes());
        encode_write(&mut bytes, &action.destination);
        bytes.push(action.baseline_word_count);
        encode_recipe(&mut bytes, &action.recipe);
    }
    encode_functions(&mut bytes, &plan.functions);
    bytes
}

fn encode_functions(bytes: &mut Vec<u8>, functions: &[crate::Aarch64MovnMaterializationFunction]) {
    encode_len(bytes, functions.len());
    for function in functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        encode_len(bytes, function.blocks.len());
        for block in &function.blocks {
            bytes.extend_from_slice(&block.block.0.to_le_bytes());
            encode_len(bytes, block.instructions.len());
            for instruction in &block.instructions {
                bytes.extend_from_slice(&instruction.instruction.0.to_le_bytes());
                match &instruction.disposition {
                    Aarch64MovnInstructionDisposition::RetainedV1 => bytes.push(0),
                    Aarch64MovnInstructionDisposition::MovnSeededMaterializationV1 {
                        literal_bits,
                        destination,
                        baseline_word_count,
                        recipe,
                    } => {
                        bytes.push(1);
                        bytes.extend_from_slice(&literal_bits.to_le_bytes());
                        encode_write(bytes, destination);
                        bytes.push(*baseline_word_count);
                        encode_recipe(bytes, recipe);
                    }
                }
            }
        }
    }
}

pub(crate) fn encode_write(bytes: &mut Vec<u8>, write: &QualifiedPhysicalWrite) {
    bytes.extend_from_slice(&write.instruction.0.to_le_bytes());
    bytes.extend_from_slice(&write.operand.to_le_bytes());
    bytes.extend_from_slice(&write.virtual_register.0.to_le_bytes());
    bytes.extend_from_slice(&write.class.0.to_le_bytes());
    bytes.extend_from_slice(&write.view.0.to_le_bytes());
    encode_units(bytes, &write.storage_units);
    encode_units(bytes, &write.write_units);
    bytes.push(match write.write_semantics {
        register_model::RegisterWriteSemantics::ExactView => 0,
        register_model::RegisterWriteSemantics::PreservesUnwritten => 1,
        register_model::RegisterWriteSemantics::ZeroExtendsParent => 2,
        register_model::RegisterWriteSemantics::ZeroExtendsWithinUnit => 3,
        register_model::RegisterWriteSemantics::Discards => 4,
        register_model::RegisterWriteSemantics::InstructionDefined => 5,
    });
}

pub(crate) fn encode_recipe(bytes: &mut Vec<u8>, recipe: &Aarch64MovnRecipe) {
    bytes.push(recipe.seed_halfword);
    bytes.extend_from_slice(&recipe.seed_immediate.to_le_bytes());
    encode_len(bytes, recipe.patches.len());
    for patch in &recipe.patches {
        bytes.push(patch.halfword);
        bytes.extend_from_slice(&patch.immediate.to_le_bytes());
    }
}

fn encode_units(bytes: &mut Vec<u8>, units: &[register_model::RegisterUnitId]) {
    encode_len(bytes, units.len());
    for unit in units {
        bytes.extend_from_slice(&unit.0.to_le_bytes());
    }
}

fn encode_len(bytes: &mut Vec<u8>, len: usize) {
    bytes.extend_from_slice(&(len as u64).to_le_bytes());
}

fn encode_target(bytes: &mut Vec<u8>, target: target::NativeTarget) {
    bytes.push(match target.architecture {
        target::Architecture::Aarch64 => 0,
        target::Architecture::X86_64 => 1,
    });
    bytes.push(match target.object_format {
        target::ObjectFormat::Elf => 0,
        target::ObjectFormat::MachO => 1,
        target::ObjectFormat::Coff => 2,
    });
    bytes.extend_from_slice(&(target.pointer_size as u64).to_le_bytes());
    bytes.extend_from_slice(&(target.pointer_alignment as u64).to_le_bytes());
}
