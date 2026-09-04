use sha2::{Digest, Sha256};

use crate::{
    X86MovR64Imm32SignExtendedInstructionDisposition,
    X86MovR64Imm32SignExtendedMaterializationIdentity,
    X86MovR64Imm32SignExtendedMaterializationPlan, X86MovR64Imm32SignExtendedMaterializationPolicy,
    X86MovR64Imm32SignExtendedMaterializationRevisionIdentity,
    X86MovR64Imm32SignExtendedPhysicalWrite,
};

const IDENTITY_DOMAIN: &[u8] = b"omega.x86-mov-r64-imm32-sign-extended-i64-materialization.v1\0";
const REVISION_DOMAIN: &[u8] =
    b"omega.x86-mov-r64-imm32-sign-extended-i64-materialization-revision.v1\0";

pub fn x86_mov_r64_imm32_sign_extended_materialization_identity(
    plan: &X86MovR64Imm32SignExtendedMaterializationPlan,
) -> X86MovR64Imm32SignExtendedMaterializationIdentity {
    let mut hasher = Sha256::new();
    hasher.update(IDENTITY_DOMAIN);
    hasher.update(encode_content(plan));
    X86MovR64Imm32SignExtendedMaterializationIdentity::from_bytes(hasher.finalize().into())
}

pub(crate) fn revision_identity(
    source: crate::PostAllocationMachineIdentity,
    selected: omega_selected_instructions::SelectedInstructionPlanIdentity,
    target: omega_target::NativeTarget,
    physical: omega_register_model::PhysicalRegisterModelIdentity,
    functions: &[crate::X86MovR64Imm32SignExtendedMaterializationFunction],
) -> X86MovR64Imm32SignExtendedMaterializationRevisionIdentity {
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
    X86MovR64Imm32SignExtendedMaterializationRevisionIdentity::from_bytes(hasher.finalize().into())
}

pub(crate) fn encode_content(plan: &X86MovR64Imm32SignExtendedMaterializationPlan) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.source.bytes());
    bytes.extend_from_slice(&plan.selected.bytes());
    encode_target(&mut bytes, plan.target);
    bytes.extend_from_slice(&plan.physical_register_model.bytes());
    bytes.push(match plan.policy {
        X86MovR64Imm32SignExtendedMaterializationPolicy::X86SelectMovR64Imm32SignExtendedI64MaterializationV1 => 0,
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
            crate::X86MovR64Imm32SignExtendedMaterializationAttemptOutcome::AlreadySelected => 0,
            crate::X86MovR64Imm32SignExtendedMaterializationAttemptOutcome::IntegerOutsideSignExtendedI32 => 1,
            crate::X86MovR64Imm32SignExtendedMaterializationAttemptOutcome::SelectedForRewrite => 2,
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
    functions: &[crate::X86MovR64Imm32SignExtendedMaterializationFunction],
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
                    X86MovR64Imm32SignExtendedInstructionDisposition::RetainedV1 => bytes.push(0),
                    X86MovR64Imm32SignExtendedInstructionDisposition::MovR64Imm32SignExtendedMaterializationV1 {
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

pub(crate) fn encode_write(bytes: &mut Vec<u8>, write: &X86MovR64Imm32SignExtendedPhysicalWrite) {
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

fn encode_write_semantics(
    bytes: &mut Vec<u8>,
    semantics: omega_register_model::RegisterWriteSemantics,
) {
    bytes.push(match semantics {
        omega_register_model::RegisterWriteSemantics::ExactView => 0,
        omega_register_model::RegisterWriteSemantics::PreservesUnwritten => 1,
        omega_register_model::RegisterWriteSemantics::ZeroExtendsParent => 2,
        omega_register_model::RegisterWriteSemantics::ZeroExtendsWithinUnit => 3,
        omega_register_model::RegisterWriteSemantics::Discards => 4,
        omega_register_model::RegisterWriteSemantics::InstructionDefined => 5,
    });
}

fn encode_units(bytes: &mut Vec<u8>, units: &[omega_register_model::RegisterUnitId]) {
    encode_len(bytes, units.len());
    for unit in units {
        bytes.extend_from_slice(&unit.0.to_le_bytes());
    }
}

fn encode_len(bytes: &mut Vec<u8>, len: usize) {
    bytes.extend_from_slice(&(len as u64).to_le_bytes());
}

fn encode_target(bytes: &mut Vec<u8>, target: omega_target::NativeTarget) {
    bytes.push(match target.architecture {
        omega_target::Architecture::Aarch64 => 0,
        omega_target::Architecture::X86_64 => 1,
    });
    bytes.push(match target.object_format {
        omega_target::ObjectFormat::Elf => 0,
        omega_target::ObjectFormat::MachO => 1,
        omega_target::ObjectFormat::Coff => 2,
    });
    bytes.extend_from_slice(&(target.pointer_size as u64).to_le_bytes());
    bytes.extend_from_slice(&(target.pointer_alignment as u64).to_le_bytes());
}
