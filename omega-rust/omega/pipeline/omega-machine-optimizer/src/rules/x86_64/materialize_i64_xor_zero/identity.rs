use sha2::{Digest, Sha256};

use crate::{
    X86XorZeroInstructionDisposition, X86XorZeroMaterializationIdentity,
    X86XorZeroMaterializationPlan, X86XorZeroMaterializationPolicy,
    X86XorZeroMaterializationRevisionIdentity, X86XorZeroPhysicalWrite,
};

const IDENTITY_DOMAIN: &[u8] = b"omega.x86-xor-zero-i64-materialization.v1\0";
const REVISION_DOMAIN: &[u8] = b"omega.x86-xor-zero-i64-materialization-revision.v1\0";

pub fn x86_xor_zero_materialization_identity(
    plan: &X86XorZeroMaterializationPlan,
) -> X86XorZeroMaterializationIdentity {
    let mut hasher = Sha256::new();
    hasher.update(IDENTITY_DOMAIN);
    hasher.update(encode_content(plan));
    X86XorZeroMaterializationIdentity::from_bytes(hasher.finalize().into())
}

pub(crate) fn revision_identity(
    source: crate::PostAllocationMachineIdentity,
    selected: omega_selected_instructions::SelectedInstructionPlanIdentity,
    liveness: omega_selected_instructions_to_register_homes::LivenessIdentity,
    target: omega_target::NativeTarget,
    physical: omega_register_model::PhysicalRegisterModelIdentity,
    functions: &[crate::X86XorZeroMaterializationFunction],
) -> X86XorZeroMaterializationRevisionIdentity {
    let mut hasher = Sha256::new();
    hasher.update(REVISION_DOMAIN);
    hasher.update(source.bytes());
    hasher.update(selected.bytes());
    hasher.update(liveness.bytes());
    let mut target_bytes = Vec::new();
    encode_target(&mut target_bytes, target);
    hasher.update(target_bytes);
    hasher.update(physical.bytes());
    let mut roster = Vec::new();
    encode_functions(&mut roster, functions);
    hasher.update(roster);
    X86XorZeroMaterializationRevisionIdentity::from_bytes(hasher.finalize().into())
}

pub(crate) fn encode_content(plan: &X86XorZeroMaterializationPlan) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.source.bytes());
    bytes.extend_from_slice(&plan.selected.bytes());
    bytes.extend_from_slice(&plan.liveness.bytes());
    encode_target(&mut bytes, plan.target);
    bytes.extend_from_slice(&plan.physical_register_model.bytes());
    bytes.push(match plan.policy {
        X86XorZeroMaterializationPolicy::X86SelectXorZeroI64MaterializationV1 => 0,
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
        encode_units(&mut bytes, &attempt.rflags_units);
        bytes.push(attempt.baseline_byte_count);
        bytes.push(attempt.selected_byte_count);
        bytes.push(match attempt.outcome {
            crate::X86XorZeroMaterializationAttemptOutcome::AlreadySelected => 0,
            crate::X86XorZeroMaterializationAttemptOutcome::NonZeroLiteral => 1,
            crate::X86XorZeroMaterializationAttemptOutcome::RflagsLiveOut => 2,
            crate::X86XorZeroMaterializationAttemptOutcome::SelectedForRewrite => 3,
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
        encode_write(&mut bytes, &action.destination);
        encode_units(&mut bytes, &action.rflags_units);
        bytes.push(action.baseline_byte_count);
        bytes.push(action.selected_byte_count);
    }
    encode_functions(&mut bytes, &plan.functions);
    bytes
}

fn encode_functions(bytes: &mut Vec<u8>, functions: &[crate::X86XorZeroMaterializationFunction]) {
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
                    X86XorZeroInstructionDisposition::RetainedV1 => bytes.push(0),
                    X86XorZeroInstructionDisposition::XorZeroMaterializationV1 {
                        destination,
                        rflags_units,
                        baseline_byte_count,
                        selected_byte_count,
                    } => {
                        bytes.push(1);
                        encode_write(bytes, destination);
                        encode_units(bytes, rflags_units);
                        bytes.push(*baseline_byte_count);
                        bytes.push(*selected_byte_count);
                    }
                }
            }
        }
    }
}

pub(crate) fn encode_write(bytes: &mut Vec<u8>, write: &X86XorZeroPhysicalWrite) {
    bytes.extend_from_slice(&write.instruction.0.to_le_bytes());
    bytes.extend_from_slice(&write.operand.to_le_bytes());
    bytes.extend_from_slice(&write.virtual_register.0.to_le_bytes());
    bytes.extend_from_slice(&write.class.0.to_le_bytes());
    bytes.extend_from_slice(&write.view.0.to_le_bytes());
    encode_units(bytes, &write.storage_units);
    encode_units(bytes, &write.write_units);
    bytes.push(match write.write_semantics {
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
