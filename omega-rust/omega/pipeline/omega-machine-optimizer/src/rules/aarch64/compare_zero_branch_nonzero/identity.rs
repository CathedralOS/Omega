use sha2::{Digest, Sha256};

use crate::{
    Aarch64CbnzFusionIdentity, Aarch64CbnzFusionPlan, Aarch64CbnzFusionPolicy,
    Aarch64CbnzFusionRevisionIdentity, Aarch64CbnzInstructionDisposition, QualifiedPhysicalRead,
};

const IDENTITY_DOMAIN: &[u8] = b"omega.aarch64-compare-zero-branch-cbnz-fusion.v1\0";
const REVISION_DOMAIN: &[u8] = b"omega.aarch64-compare-zero-branch-cbnz-fusion-revision.v1\0";

pub fn aarch64_cbnz_fusion_identity(plan: &Aarch64CbnzFusionPlan) -> Aarch64CbnzFusionIdentity {
    let mut hasher = Sha256::new();
    hasher.update(IDENTITY_DOMAIN);
    hasher.update(encode_content(plan));
    Aarch64CbnzFusionIdentity::from_bytes(hasher.finalize().into())
}

pub(crate) fn revision_identity(
    source: crate::PostAllocationMachineIdentity,
    selected: omega_selected_instructions::SelectedInstructionPlanIdentity,
    liveness: omega_selected_instructions_to_register_homes::LivenessIdentity,
    target: omega_target::NativeTarget,
    physical: omega_register_model::PhysicalRegisterModelIdentity,
    functions: &[crate::Aarch64CbnzFusionFunction],
) -> Aarch64CbnzFusionRevisionIdentity {
    let mut hasher = Sha256::new();
    hasher.update(REVISION_DOMAIN);
    encode_roots(&mut hasher, source, selected, liveness, target, physical);
    encode_functions(&mut hasher, functions);
    Aarch64CbnzFusionRevisionIdentity::from_bytes(hasher.finalize().into())
}

pub(crate) fn encode_content(plan: &Aarch64CbnzFusionPlan) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.source.bytes());
    bytes.extend_from_slice(&plan.selected.bytes());
    bytes.extend_from_slice(&plan.liveness.bytes());
    encode_target(&mut bytes, plan.target);
    bytes.extend_from_slice(&plan.physical_register_model.bytes());
    bytes.push(match plan.policy {
        Aarch64CbnzFusionPolicy::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 => 0,
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
        bytes.extend_from_slice(&attempt.compare.0.to_le_bytes());
        bytes.extend_from_slice(&attempt.branch.0.to_le_bytes());
        bytes.push(match attempt.outcome {
            crate::Aarch64CbnzFusionAttemptOutcome::AlreadyFused => 0,
            crate::Aarch64CbnzFusionAttemptOutcome::CompareCarriesFuel => 1,
            crate::Aarch64CbnzFusionAttemptOutcome::NzcvLiveOut => 2,
            crate::Aarch64CbnzFusionAttemptOutcome::SelectedForFusion => 3,
        });
    }
    encode_len(&mut bytes, plan.actions.len());
    for action in &plan.actions {
        bytes.extend_from_slice(&action.iteration.to_le_bytes());
        bytes.extend_from_slice(&action.input.bytes());
        bytes.extend_from_slice(&action.output.bytes());
        bytes.extend_from_slice(&action.machine.get().to_le_bytes());
        bytes.extend_from_slice(&action.block.0.to_le_bytes());
        bytes.extend_from_slice(&action.compare.0.to_le_bytes());
        bytes.extend_from_slice(&action.branch.0.to_le_bytes());
        encode_read(&mut bytes, &action.source_read);
        encode_units(&mut bytes, &action.nzcv_units);
        encode_units(&mut bytes, &action.pc_units);
        bytes.extend_from_slice(&action.when_nonzero_edge.get().to_le_bytes());
        bytes.extend_from_slice(&action.when_nonzero_block.0.to_le_bytes());
        bytes.extend_from_slice(&action.when_zero_edge.get().to_le_bytes());
        bytes.extend_from_slice(&action.when_zero_block.0.to_le_bytes());
    }
    encode_functions_to_bytes(&mut bytes, &plan.functions);
    bytes
}

fn encode_roots(
    hasher: &mut Sha256,
    source: crate::PostAllocationMachineIdentity,
    selected: omega_selected_instructions::SelectedInstructionPlanIdentity,
    liveness: omega_selected_instructions_to_register_homes::LivenessIdentity,
    target: omega_target::NativeTarget,
    physical: omega_register_model::PhysicalRegisterModelIdentity,
) {
    hasher.update(source.bytes());
    hasher.update(selected.bytes());
    hasher.update(liveness.bytes());
    let mut encoded = Vec::new();
    encode_target(&mut encoded, target);
    hasher.update(encoded);
    hasher.update(physical.bytes());
}

fn encode_functions(hasher: &mut Sha256, functions: &[crate::Aarch64CbnzFusionFunction]) {
    let mut encoded = Vec::new();
    encode_functions_to_bytes(&mut encoded, functions);
    hasher.update(encoded);
}

fn encode_functions_to_bytes(bytes: &mut Vec<u8>, functions: &[crate::Aarch64CbnzFusionFunction]) {
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
                    Aarch64CbnzInstructionDisposition::RetainedV1 => bytes.push(0),
                    Aarch64CbnzInstructionDisposition::ElidedCompareI64ZeroV1 { consumer } => {
                        bytes.push(1);
                        bytes.extend_from_slice(&consumer.0.to_le_bytes());
                    }
                    Aarch64CbnzInstructionDisposition::FusedBranchNonZeroToCbnzV1 {
                        compare,
                        source_read,
                    } => {
                        bytes.push(2);
                        bytes.extend_from_slice(&compare.0.to_le_bytes());
                        encode_read(bytes, source_read);
                    }
                }
            }
        }
    }
}

fn encode_read(bytes: &mut Vec<u8>, read: &QualifiedPhysicalRead) {
    bytes.extend_from_slice(&read.source_instruction.0.to_le_bytes());
    bytes.extend_from_slice(&read.operand.to_le_bytes());
    bytes.extend_from_slice(&read.virtual_register.0.to_le_bytes());
    bytes.extend_from_slice(&read.class.0.to_le_bytes());
    bytes.extend_from_slice(&read.view.0.to_le_bytes());
    encode_units(bytes, &read.units);
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
