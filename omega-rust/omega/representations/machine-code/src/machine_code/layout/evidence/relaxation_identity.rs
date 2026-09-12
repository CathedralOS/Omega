use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior,
};
use sha2::{Digest, Sha256};
use target::{Architecture, NativeTarget, ObjectFormat};

use crate::{
    ResolvedConditionalBranchPredicate, ResolvedSelectedFormLayoutIdentity,
    ResolvedSelectedFunctionLayout,
};

use super::{
    X86BranchRelaxationAction, X86BranchRelaxationAttempt, X86BranchRelaxationAttemptOutcome,
    X86BranchRelaxationIdentity, X86BranchRelaxationPolicy, X86BranchRelaxationRevisionIdentity,
};

const RELAXATION_SCHEMA: &[u8] = b"omega.terminal.x86-branch-relaxation.v9";
const REVISION_SCHEMA: &[u8] = b"omega.terminal.x86-branch-relaxation-revision.v3";

#[derive(Clone, Copy)]
pub struct RevisionRoots {
    pub source: ResolvedSelectedFormLayoutIdentity,
    pub selected: selected_instructions::SelectedInstructionPlanIdentity,
    pub machine: physical_instructions::PostAllocationMachineIdentity,
    pub pre_layout: crate::SelectedFormEncodingIdentity,
    pub target: NativeTarget,
}

pub fn revision_identity(
    roots: RevisionRoots,
    functions: &[ResolvedSelectedFunctionLayout],
) -> X86BranchRelaxationRevisionIdentity {
    let mut hasher = Sha256::new();
    hasher.update(REVISION_SCHEMA);
    encode_roots(&mut hasher, roots);
    encode_functions(&mut hasher, functions);
    X86BranchRelaxationRevisionIdentity::from_bytes(hasher.finalize().into())
}

#[allow(clippy::too_many_arguments)]
pub fn artifact_identity(
    roots: RevisionRoots,
    policy: X86BranchRelaxationPolicy,
    budget: OptimizationWorkBudget,
    usage: OptimizationWorkUsage,
    output: ResolvedSelectedFormLayoutIdentity,
    output_revision: X86BranchRelaxationRevisionIdentity,
    attempts: &[X86BranchRelaxationAttempt],
    actions: &[X86BranchRelaxationAction],
    functions: &[ResolvedSelectedFunctionLayout],
) -> X86BranchRelaxationIdentity {
    let mut hasher = Sha256::new();
    hasher.update(RELAXATION_SCHEMA);
    encode_roots(&mut hasher, roots);
    hasher.update([match policy {
        X86BranchRelaxationPolicy::X86RelaxConditionalBranchesToRel8V1 => 0,
    }]);
    hasher.update(budget.encode());
    hasher.update(usage.encode());
    hasher.update(output.bytes());
    hasher.update(output_revision.bytes());
    hasher.update((attempts.len() as u64).to_le_bytes());
    for attempt in attempts {
        hasher.update(attempt.iteration.to_le_bytes());
        hasher.update(attempt.input.bytes());
        hasher.update(attempt.instruction.0.to_le_bytes());
        hasher.update(attempt.offset.to_le_bytes());
        hasher.update(attempt.byte_displacement.to_le_bytes());
        hasher.update([attempt.encoded_bytes]);
        hasher.update([match attempt.outcome {
            X86BranchRelaxationAttemptOutcome::AlreadyShort => 0,
            X86BranchRelaxationAttemptOutcome::NearDisplacementOutsideI8 => 1,
            X86BranchRelaxationAttemptOutcome::SelectedForRelaxation => 2,
        }]);
    }
    hasher.update((actions.len() as u64).to_le_bytes());
    for action in actions {
        hasher.update(action.iteration.to_le_bytes());
        hasher.update(action.input.bytes());
        hasher.update(action.output.bytes());
        hasher.update(action.instruction.0.to_le_bytes());
        hasher.update(action.old_offset.to_le_bytes());
        hasher.update(action.new_offset.to_le_bytes());
        hasher.update(action.old_displacement.to_le_bytes());
        hasher.update(action.new_displacement.to_le_bytes());
        hasher.update((action.old_bytes.len() as u64).to_le_bytes());
        hasher.update(&action.old_bytes);
        hasher.update((action.new_bytes.len() as u64).to_le_bytes());
        hasher.update(&action.new_bytes);
    }
    encode_functions(&mut hasher, functions);
    X86BranchRelaxationIdentity::from_bytes(hasher.finalize().into())
}

fn encode_roots(hasher: &mut Sha256, roots: RevisionRoots) {
    hasher.update(roots.source.bytes());
    hasher.update(roots.selected.bytes());
    hasher.update(roots.machine.bytes());
    hasher.update(roots.pre_layout.bytes());
    hasher.update([match roots.target.architecture {
        Architecture::Aarch64 => 0,
        Architecture::X86_64 => 1,
    }]);
    hasher.update([match roots.target.object_format {
        ObjectFormat::Elf => 0,
        ObjectFormat::MachO => 1,
        ObjectFormat::Coff => 2,
    }]);
    hasher.update((roots.target.pointer_size as u64).to_le_bytes());
    hasher.update((roots.target.pointer_alignment as u64).to_le_bytes());
}

fn encode_functions(hasher: &mut Sha256, functions: &[ResolvedSelectedFunctionLayout]) {
    hasher.update((functions.len() as u64).to_le_bytes());
    for function in functions {
        hasher.update(function.machine.get().to_le_bytes());
        hasher.update(function.byte_count.to_le_bytes());
        hasher.update((function.blocks.len() as u64).to_le_bytes());
        for block in &function.blocks {
            hasher.update(block.block.0.to_le_bytes());
            hasher.update(block.offset.to_le_bytes());
            hasher.update(block.byte_count.to_le_bytes());
            hasher.update((block.instructions.len() as u64).to_le_bytes());
            for row in &block.instructions {
                hasher.update(row.instruction.0.to_le_bytes());
                encode_alternative(hasher, row.alternative);
                hasher.update(row.offset.to_le_bytes());
                hasher.update((row.bytes.len() as u64).to_le_bytes());
                hasher.update(&row.bytes);
                match row.branch.as_deref() {
                    None => hasher.update([0]),
                    Some(crate::ResolvedBranchEvidence::Conditional(branch)) => {
                        hasher.update([1]);
                        hasher.update([match branch.predicate {
                            ResolvedConditionalBranchPredicate::NonZeroV1 => 0,
                            ResolvedConditionalBranchPredicate::U64LessThanV1 => 1,
                            ResolvedConditionalBranchPredicate::I64LessThanV1 => 2,
                        }]);
                        hasher.update(branch.source_block.0.to_le_bytes());
                        hasher.update(branch.when_taken_edge.get().to_le_bytes());
                        hasher.update(branch.when_taken_block.0.to_le_bytes());
                        hasher.update(branch.when_taken_offset.to_le_bytes());
                        hasher.update(branch.when_fallthrough_edge.get().to_le_bytes());
                        hasher.update(branch.when_fallthrough_block.0.to_le_bytes());
                        hasher.update(branch.when_fallthrough_offset.to_le_bytes());
                        hasher.update(branch.byte_displacement.to_le_bytes());
                        encode_effects(hasher, &branch.decoded_effects);
                    }
                    Some(crate::ResolvedBranchEvidence::Jump(jump)) => {
                        hasher.update([2]);
                        hasher.update(jump.source_block.0.to_le_bytes());
                        hasher.update(jump.target_edge.get().to_le_bytes());
                        hasher.update(jump.target_block.0.to_le_bytes());
                        hasher.update(jump.target_offset.to_le_bytes());
                        hasher.update(jump.byte_displacement.to_le_bytes());
                        encode_effects(hasher, &jump.decoded_effects);
                    }
                }
            }
        }
    }
}

fn encode_alternative(hasher: &mut Sha256, alternative: MachineAlternativeKey) {
    hasher.update([match alternative.family {
        MachineAlternativeFamily::CompareI64Zero => 0,
        MachineAlternativeFamily::MaterializeI64 => 1,
        MachineAlternativeFamily::CopyI64 => 2,
        MachineAlternativeFamily::Float32ToBits => 26,
        MachineAlternativeFamily::Float64ToBits => 27,
        MachineAlternativeFamily::BitsToFloat32 => 28,
        MachineAlternativeFamily::BitsToFloat64 => 29,
        MachineAlternativeFamily::ZeroExtendU8 => 15,
        MachineAlternativeFamily::ZeroExtendU32 => 20,
        MachineAlternativeFamily::ZeroExtendU16 => 37,
        MachineAlternativeFamily::SignExtendI8 => 38,
        MachineAlternativeFamily::SignExtendI16 => 39,
        MachineAlternativeFamily::SignExtendI32 => 40,
        MachineAlternativeFamily::MaterializeBooleanEqual => 41,
        MachineAlternativeFamily::MaterializeBooleanU64LessThan => 42,
        MachineAlternativeFamily::MaterializeBooleanI64LessThan => 43,
        MachineAlternativeFamily::MaterializeBooleanU64LessOrEqual => 44,
        MachineAlternativeFamily::MaterializeBooleanI64LessOrEqual => 45,
        MachineAlternativeFamily::ExactAddI64 => 3,
        MachineAlternativeFamily::ExactAddI64Immediate => 4,
        MachineAlternativeFamily::ExactSubtractI64 => 5,
        MachineAlternativeFamily::ConditionalBranchNonZero => 6,
        MachineAlternativeFamily::ReturnScalar => 7,
        MachineAlternativeFamily::ExactSubtractI64Immediate => 8,
        MachineAlternativeFamily::ReturnUnit => 9,
        MachineAlternativeFamily::CompareI64 => 10,
        MachineAlternativeFamily::ConditionalBranchU64LessThan => 11,
        MachineAlternativeFamily::ConditionalBranchI64LessThan => 12,
        MachineAlternativeFamily::CallScalar => 13,
        MachineAlternativeFamily::Jump => 14,
        MachineAlternativeFamily::Store => 24,
        MachineAlternativeFamily::AddressOffset => 25,
        MachineAlternativeFamily::Load64 => 16,
        MachineAlternativeFamily::LoadPacked3 => 46,
        MachineAlternativeFamily::LoadPacked5 => 47,
        MachineAlternativeFamily::LoadPacked6 => 48,
        MachineAlternativeFamily::LoadPacked7 => 49,
        MachineAlternativeFamily::StorePacked => 50,
        MachineAlternativeFamily::BitwiseAndI64 => 51,
        MachineAlternativeFamily::BitwiseXorI64 => 52,
        MachineAlternativeFamily::Load8 => 33,
        MachineAlternativeFamily::Load16 => 34,
        MachineAlternativeFamily::Load32 => 30,
        MachineAlternativeFamily::HostedExitProcessI32 => 31,
        MachineAlternativeFamily::HostedReadByte => 32,
        MachineAlternativeFamily::HostedWriteByteI32 => 23,
        MachineAlternativeFamily::ByteViewAddress => 22,
        MachineAlternativeFamily::Load8Indexed => 21,
        MachineAlternativeFamily::Store64 => 17,
        MachineAlternativeFamily::FrameAddress => 18,
        MachineAlternativeFamily::CallUnit => 19,
        MachineAlternativeFamily::CallAggregate => 35,
        MachineAlternativeFamily::ReturnAggregate => 36,
    }]);
    hasher.update(alternative.variant.to_le_bytes());
}

fn encode_effects(hasher: &mut Sha256, effects: &MachineEncodedEffects) {
    encode_u16s(hasher, &effects.external_operand_reads);
    encode_u16s(hasher, &effects.external_operand_writes);
    encode_units(hasher, &effects.implicit_unit_uses);
    encode_units(hasher, &effects.implicit_unit_defs);
    encode_units(hasher, &effects.implicit_unit_clobbers);
    match effects.memory {
        MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
            pointer_operand,
            index_operand,
            byte_count,
        } => {
            hasher.update([5]);
            hasher.update(pointer_operand.to_le_bytes());
            hasher.update(index_operand.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
        MachineEncodedMemoryEffect::HostedReadByteV1 { stack_pointer } => {
            hasher.update([8]);
            hasher.update(stack_pointer.0.to_le_bytes());
        }
        MachineEncodedMemoryEffect::HostedWriteByteV1 { stack_pointer } => {
            hasher.update([6]);
            hasher.update(stack_pointer.0.to_le_bytes());
        }
        MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand } => {
            hasher.update([7]);
            hasher.update(pointer_operand.to_le_bytes());
        }
        MachineEncodedMemoryEffect::NoneV1 => hasher.update([0]),
        MachineEncodedMemoryEffect::ReadPointerV1 {
            pointer_operand,
            byte_count,
        } => {
            hasher.update([3]);
            hasher.update(pointer_operand.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
        MachineEncodedMemoryEffect::WriteFrameStorageV1 {
            stack_pointer,
            byte_count,
        } => {
            hasher.update([4]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
        MachineEncodedMemoryEffect::ReadActivationStackV1 {
            stack_pointer,
            byte_count,
        } => {
            hasher.update([1]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
        MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
            stack_pointer,
            byte_count,
        } => {
            hasher.update([2]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
    }
    match effects.stack {
        MachineEncodedStackEffect::UnchangedV1 => hasher.update([0]),
        MachineEncodedStackEffect::PopBytesV1 {
            stack_pointer,
            byte_count,
        } => {
            hasher.update([1]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
        MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
            stack_pointer,
            return_address_byte_count,
        } => {
            hasher.update([2]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(return_address_byte_count.to_le_bytes());
        }
    }
    hasher.update([match effects.trap {
        MachineEncodedTrapBehavior::NeverV1 => 0,
        MachineEncodedTrapBehavior::HostedExitReturnedV1 => 3,
        MachineEncodedTrapBehavior::HostedReadFailureV1 => 4,
        MachineEncodedTrapBehavior::HostedWriteFailureV1 => 2,
        MachineEncodedTrapBehavior::MayArchitecturalFaultV1 => 1,
    }]);
    match effects.control {
        MachineEncodedControlEffect::HostedExitOrTrapV1 => hasher.update([7]),
        MachineEncodedControlEffect::HostedReadReturnOrTrapV1 => hasher.update([8]),
        MachineEncodedControlEffect::HostedWriteReturnOrTrapV1 => hasher.update([6]),
        MachineEncodedControlEffect::FallThroughV1 => hasher.update([0]),
        MachineEncodedControlEffect::ConditionalRelativeBranchV1 => hasher.update([1]),
        MachineEncodedControlEffect::ReturnFromActivationStackV1 => hasher.update([2]),
        MachineEncodedControlEffect::ReturnIndirectRegisterV1 { target } => {
            hasher.update([3]);
            hasher.update(target.0.to_le_bytes());
        }
        MachineEncodedControlEffect::DirectRelativeCallV1 => hasher.update([4]),
        MachineEncodedControlEffect::UnconditionalRelativeBranchV1 => hasher.update([5]),
    }
}

fn encode_u16s(hasher: &mut Sha256, values: &[u16]) {
    hasher.update((values.len() as u64).to_le_bytes());
    for value in values {
        hasher.update(value.to_le_bytes());
    }
}

fn encode_units(hasher: &mut Sha256, values: &[register_model::RegisterUnitId]) {
    hasher.update((values.len() as u64).to_le_bytes());
    for value in values {
        hasher.update(value.0.to_le_bytes());
    }
}
