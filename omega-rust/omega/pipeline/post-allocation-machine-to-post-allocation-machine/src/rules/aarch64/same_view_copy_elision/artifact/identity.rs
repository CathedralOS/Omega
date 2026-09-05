use sha2::{Digest, Sha256};

use crate::{
    Aarch64SameViewCopyElisionIdentity, Aarch64SameViewCopyElisionPlan,
    Aarch64SameViewCopyElisionRevisionIdentity, Aarch64SameViewCopyInstructionDisposition,
    QualifiedPhysicalOperand,
};

const IDENTITY_DOMAIN: &[u8] = b"omega.aarch64-same-view-copy-elision.v4\0";
const REVISION_DOMAIN: &[u8] = b"omega.aarch64-same-view-copy-elision-revision.v2\0";

pub fn aarch64_same_view_copy_elision_identity(
    plan: &Aarch64SameViewCopyElisionPlan,
) -> Aarch64SameViewCopyElisionIdentity {
    let mut hasher = Sha256::new();
    hasher.update(IDENTITY_DOMAIN);
    hasher.update(encode_content(plan));
    Aarch64SameViewCopyElisionIdentity::from_bytes(hasher.finalize().into())
}

pub(crate) fn revision_identity(
    source: physical_instructions::PostAllocationMachineIdentity,
    selected: selected_instructions::SelectedInstructionPlanIdentity,
    liveness: selected_instructions_to_register_homes::LivenessIdentity,
    target: target::NativeTarget,
    physical: register_model::PhysicalRegisterModelIdentity,
    functions: &[crate::Aarch64SameViewCopyElisionFunction],
) -> Aarch64SameViewCopyElisionRevisionIdentity {
    let mut hasher = Sha256::new();
    hasher.update(REVISION_DOMAIN);
    hasher.update(source.bytes());
    hasher.update(selected.bytes());
    hasher.update(liveness.bytes());
    encode_target(&mut hasher, target);
    hasher.update(physical.bytes());
    encode_functions(&mut hasher, functions);
    Aarch64SameViewCopyElisionRevisionIdentity::from_bytes(hasher.finalize().into())
}

pub(super) fn encode_content(plan: &Aarch64SameViewCopyElisionPlan) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.source.bytes());
    bytes.extend_from_slice(&plan.selected.bytes());
    bytes.extend_from_slice(&plan.liveness.bytes());
    encode_target(&mut bytes, plan.target);
    bytes.extend_from_slice(&plan.physical_register_model.bytes());
    bytes.push(match plan.policy {
        crate::Aarch64SameViewCopyElisionPolicy::Aarch64ElideSameViewCopyI64BeforeReturnV1 => 0,
        crate::Aarch64SameViewCopyElisionPolicy::Aarch64ElideSameViewCopyI64BeforeCompareZeroV1 => {
            1
        }
        crate::Aarch64SameViewCopyElisionPolicy::Aarch64ElideSameViewCopyI64BeforeCompareI64LeftOperandV1 => 2,
        crate::Aarch64SameViewCopyElisionPolicy::Aarch64ElideSameViewCopyI64BeforeCompareI64RightOperandV1 => 3,
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
        bytes.extend_from_slice(&attempt.copy.0.to_le_bytes());
        bytes.extend_from_slice(&attempt.consumer.0.to_le_bytes());
        bytes.push(match attempt.outcome {
            crate::Aarch64SameViewCopyElisionAttemptOutcome::AlreadyElided => 0,
            crate::Aarch64SameViewCopyElisionAttemptOutcome::DifferentPhysicalStorage => 1,
            crate::Aarch64SameViewCopyElisionAttemptOutcome::DestinationNotConsumed => 2,
            crate::Aarch64SameViewCopyElisionAttemptOutcome::SemanticProvenance => 3,
            crate::Aarch64SameViewCopyElisionAttemptOutcome::SelectedForElision => 4,
        });
    }
    encode_len(&mut bytes, plan.actions.len());
    for action in &plan.actions {
        bytes.extend_from_slice(&action.iteration.to_le_bytes());
        bytes.extend_from_slice(&action.input.bytes());
        bytes.extend_from_slice(&action.output.bytes());
        bytes.extend_from_slice(&action.machine.get().to_le_bytes());
        bytes.extend_from_slice(&action.block.0.to_le_bytes());
        bytes.extend_from_slice(&action.copy.0.to_le_bytes());
        bytes.extend_from_slice(&action.consumer.0.to_le_bytes());
        encode_operand(&mut bytes, &action.source);
        encode_operand(&mut bytes, &action.destination);
        encode_operand(&mut bytes, &action.consumed);
        bytes.extend_from_slice(&action.source_value.get().to_le_bytes());
    }
    encode_functions_to_bytes(&mut bytes, &plan.functions);
    bytes
}

fn encode_target(output: &mut impl IdentitySink, target: target::NativeTarget) {
    output.write(&[match target.architecture {
        target::Architecture::Aarch64 => 0,
        target::Architecture::X86_64 => 1,
    }]);
    output.write(&[match target.object_format {
        target::ObjectFormat::Elf => 0,
        target::ObjectFormat::MachO => 1,
        target::ObjectFormat::Coff => 2,
    }]);
    output.write(&(target.pointer_size as u64).to_le_bytes());
    output.write(&(target.pointer_alignment as u64).to_le_bytes());
}

fn encode_functions(
    output: &mut impl IdentitySink,
    functions: &[crate::Aarch64SameViewCopyElisionFunction],
) {
    let mut bytes = Vec::new();
    encode_functions_to_bytes(&mut bytes, functions);
    output.write(&bytes);
}

fn encode_functions_to_bytes(
    bytes: &mut Vec<u8>,
    functions: &[crate::Aarch64SameViewCopyElisionFunction],
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
                match instruction.disposition {
                    Aarch64SameViewCopyInstructionDisposition::RetainedV1 => bytes.push(0),
                    Aarch64SameViewCopyInstructionDisposition::ElidedSameViewCopyI64V1 {
                        consumer,
                    } => {
                        bytes.push(1);
                        bytes.extend_from_slice(&consumer.0.to_le_bytes());
                    }
                }
            }
        }
    }
}

fn encode_operand(bytes: &mut Vec<u8>, operand: &QualifiedPhysicalOperand) {
    bytes.extend_from_slice(&operand.instruction.0.to_le_bytes());
    bytes.extend_from_slice(&operand.operand.to_le_bytes());
    bytes.extend_from_slice(&operand.virtual_register.0.to_le_bytes());
    bytes.extend_from_slice(&operand.class.0.to_le_bytes());
    bytes.extend_from_slice(&operand.view.0.to_le_bytes());
    encode_len(bytes, operand.storage_units.len());
    for unit in &operand.storage_units {
        bytes.extend_from_slice(&unit.0.to_le_bytes());
    }
}

fn encode_len(bytes: &mut Vec<u8>, len: usize) {
    bytes.extend_from_slice(&(len as u64).to_le_bytes());
}

trait IdentitySink {
    fn write(&mut self, bytes: &[u8]);
}

impl IdentitySink for Vec<u8> {
    fn write(&mut self, bytes: &[u8]) {
        self.extend_from_slice(bytes);
    }
}

impl IdentitySink for Sha256 {
    fn write(&mut self, bytes: &[u8]) {
        self.update(bytes);
    }
}
