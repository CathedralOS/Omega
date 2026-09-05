use sha2::{Digest, Sha256};

use crate::{
    X86MovR32Imm32InstructionDisposition, X86MovR32Imm32MaterializationIdentity,
    X86MovR32Imm32MaterializationPlan, X86MovR32Imm32MaterializationPolicy,
    X86MovR32Imm32MaterializationRevisionIdentity, X86MovR32Imm32PhysicalWrite,
};

const IDENTITY_DOMAIN: &[u8] = b"omega.x86-mov-r32-imm32-i64-materialization.v1\0";
const REVISION_DOMAIN: &[u8] = b"omega.x86-mov-r32-imm32-i64-materialization-revision.v1\0";

pub fn x86_mov_r32_imm32_materialization_identity(
    plan: &X86MovR32Imm32MaterializationPlan,
) -> X86MovR32Imm32MaterializationIdentity {
    let mut hasher = Sha256::new();
    hasher.update(IDENTITY_DOMAIN);
    hasher.update(encode_content(plan));
    X86MovR32Imm32MaterializationIdentity::from_bytes(hasher.finalize().into())
}

pub(crate) fn revision_identity(
    source: physical_instructions::PostAllocationMachineIdentity,
    selected: selected_instructions::SelectedInstructionPlanIdentity,
    target: target::NativeTarget,
    physical: register_model::PhysicalRegisterModelIdentity,
    functions: &[crate::X86MovR32Imm32MaterializationFunction],
) -> X86MovR32Imm32MaterializationRevisionIdentity {
    let mut hasher = Sha256::new();
    hasher.update(REVISION_DOMAIN);
    hasher.update(source.bytes());
    hasher.update(selected.bytes());
    let mut target_bytes = Vec::new();
    encode_target(&mut target_bytes, target);
    hasher.update(target_bytes);
    hasher.update(physical.bytes());
    let mut roster = Vec::new();
    encode_functions(&mut roster, functions);
    hasher.update(roster);
    X86MovR32Imm32MaterializationRevisionIdentity::from_bytes(hasher.finalize().into())
}

pub(crate) fn encode_content(plan: &X86MovR32Imm32MaterializationPlan) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.source.bytes());
    bytes.extend_from_slice(&plan.selected.bytes());
    encode_target(&mut bytes, plan.target);
    bytes.extend_from_slice(&plan.physical_register_model.bytes());
    bytes.push(match plan.policy {
        X86MovR32Imm32MaterializationPolicy::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1 => 0,
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
        bytes.push(attempt.baseline_byte_count);
        bytes.push(attempt.selected_byte_count);
        bytes.push(match attempt.outcome {
            crate::X86MovR32Imm32MaterializationAttemptOutcome::AlreadySelected => 0,
            crate::X86MovR32Imm32MaterializationAttemptOutcome::IntegerOutsideZeroExtendedU32 => 1,
            crate::X86MovR32Imm32MaterializationAttemptOutcome::SelectedForRewrite => 2,
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
        bytes.push(action.baseline_byte_count);
        bytes.push(action.selected_byte_count);
    }
    encode_functions(&mut bytes, &plan.functions);
    bytes
}

fn encode_functions(
    bytes: &mut Vec<u8>,
    functions: &[crate::X86MovR32Imm32MaterializationFunction],
) {
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
                    X86MovR32Imm32InstructionDisposition::RetainedV1 => bytes.push(0),
                    X86MovR32Imm32InstructionDisposition::MovR32Imm32MaterializationV1 {
                        literal_bits,
                        destination,
                        baseline_byte_count,
                        selected_byte_count,
                    } => {
                        bytes.push(1);
                        bytes.extend_from_slice(&literal_bits.to_le_bytes());
                        encode_write(bytes, destination);
                        bytes.push(*baseline_byte_count);
                        bytes.push(*selected_byte_count);
                    }
                }
            }
        }
    }
}

pub(crate) fn encode_write(bytes: &mut Vec<u8>, write: &X86MovR32Imm32PhysicalWrite) {
    bytes.extend_from_slice(&write.instruction.0.to_le_bytes());
    bytes.extend_from_slice(&write.operand.to_le_bytes());
    bytes.extend_from_slice(&write.virtual_register.0.to_le_bytes());
    bytes.extend_from_slice(&write.class.0.to_le_bytes());
    bytes.extend_from_slice(&write.destination_view.0.to_le_bytes());
    encode_units(bytes, &write.destination_storage_units);
    encode_units(bytes, &write.destination_write_units);
    encode_write_semantics(bytes, write.destination_write_semantics);
    bytes.extend_from_slice(&write.encoded_view.0.to_le_bytes());
    encode_units(bytes, &write.encoded_storage_units);
    encode_units(bytes, &write.encoded_write_units);
    encode_write_semantics(bytes, write.encoded_write_semantics);
}

fn encode_write_semantics(bytes: &mut Vec<u8>, semantics: register_model::RegisterWriteSemantics) {
    bytes.push(match semantics {
        register_model::RegisterWriteSemantics::ExactView => 0,
        register_model::RegisterWriteSemantics::PreservesUnwritten => 1,
        register_model::RegisterWriteSemantics::ZeroExtendsParent => 2,
        register_model::RegisterWriteSemantics::ZeroExtendsWithinUnit => 3,
        register_model::RegisterWriteSemantics::Discards => 4,
        register_model::RegisterWriteSemantics::InstructionDefined => 5,
    });
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
