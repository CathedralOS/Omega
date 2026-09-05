//! Canonical identity of the complete fragment program, including zero-code rows.

use super::{
    FunctionFragmentConditionalBranchEvidence, FunctionFragmentConditionalBranchPredicate,
    FunctionFragmentControlProvenance, FunctionFragmentEmissionPlan,
    FunctionFragmentInstructionSpan, FunctionFragmentInternalMachineFixup,
    FunctionFragmentInternalMachineFixupKind, FunctionFragmentInternalMachineFixupState,
    FunctionFragmentSuccessorProvenance,
};
use optimization_core::FunctionFragmentEmissionIdentity;
use optimization_unit::{FuelSettlement, PsiProvenance};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, SelectedInstructionProvenance,
};
use semantic_vocabulary::{IntegerCarrier, IntegerSign, ScalarType};
use sha2::{Digest, Sha256};
use target::{Architecture, NativeTarget, ObjectFormat};
use target_operations::TerminalPsiProvenance;

const FRAGMENT_SCHEMA: &[u8] = b"omega.terminal.function-fragment-emission.v5";

pub fn function_fragment_emission_identity(
    plan: &FunctionFragmentEmissionPlan,
) -> FunctionFragmentEmissionIdentity {
    let mut hasher = Sha256::new();
    hasher.update(FRAGMENT_SCHEMA);
    hasher.update(plan.psi.vocabulary_marker.get().to_le_bytes());
    hasher.update(plan.psi.program_fingerprint.as_bytes());
    hasher.update(plan.fuel_schedule.marker().to_le_bytes());
    hasher.update(plan.selected.bytes());
    encode_target(&mut hasher, plan.target);
    hasher.update(plan.entry.get().to_le_bytes());
    hasher.update((plan.functions.len() as u64).to_le_bytes());
    for function in &plan.functions {
        hasher.update(function.machine.get().to_le_bytes());
        match function.attachment {
            None => hasher.update([0]),
            Some(attachment) => {
                hasher.update([1]);
                hasher.update(attachment.get().to_le_bytes());
            }
        }
        encode_function_provenance(&mut hasher, &function.provenance);
        hasher.update(function.byte_count.to_le_bytes());
        encode_bytes(&mut hasher, &function.bytes);
        hasher.update((function.blocks.len() as u64).to_le_bytes());
        for block in &function.blocks {
            hasher.update(block.block.0.to_le_bytes());
            hasher.update(block.offset.to_le_bytes());
            hasher.update(block.byte_count.to_le_bytes());
            hasher.update((block.instructions.len() as u64).to_le_bytes());
            for row in &block.instructions {
                hasher.update(row.instruction.0.to_le_bytes());
                encode_alternative(&mut hasher, row.alternative);
                hasher.update(row.offset.to_le_bytes());
                encode_bytes(&mut hasher, &row.bytes);
                encode_branch(&mut hasher, row.branch.as_deref());
                encode_optional_internal_machine_fixup(&mut hasher, row.internal_machine_fixup);
                encode_instruction_provenance(&mut hasher, &row.provenance);
                encode_control(&mut hasher, &row.control);
            }
        }
    }
    hasher.update((plan.structural_unit_functions.len() as u64).to_le_bytes());
    for function in &plan.structural_unit_functions {
        hasher.update(function.machine.get().to_le_bytes());
        match function.attachment {
            None => hasher.update([0]),
            Some(attachment) => {
                hasher.update([1]);
                hasher.update(attachment.get().to_le_bytes());
            }
        }
        encode_function_provenance(&mut hasher, &function.provenance);
        hasher.update(function.byte_count.to_le_bytes());
        encode_bytes(&mut hasher, &function.bytes);
        let block = &function.block;
        hasher.update(block.block.0.to_le_bytes());
        hasher.update(block.offset.to_le_bytes());
        hasher.update(block.byte_count.to_le_bytes());
        match &block.call {
            None => hasher.update([0]),
            Some(call) => {
                hasher.update([1]);
                hasher.update(call.instruction.0.to_le_bytes());
                hasher.update(call.operation.get().to_le_bytes());
                hasher.update(call.callee.get().to_le_bytes());
                hasher.update(call.offset.to_le_bytes());
                encode_bytes(&mut hasher, &call.bytes);
                encode_instruction_provenance(&mut hasher, &call.provenance);
                encode_internal_machine_fixup(&mut hasher, call.fixup);
            }
        }
        encode_instruction_span(&mut hasher, &block.return_instruction);
    }
    FunctionFragmentEmissionIdentity::from_canonical_bytes(&hasher.finalize())
}

fn encode_instruction_span(hasher: &mut Sha256, row: &FunctionFragmentInstructionSpan) {
    hasher.update(row.instruction.0.to_le_bytes());
    encode_alternative(hasher, row.alternative);
    hasher.update(row.offset.to_le_bytes());
    encode_bytes(hasher, &row.bytes);
    encode_branch(hasher, row.branch.as_deref());
    encode_optional_internal_machine_fixup(hasher, row.internal_machine_fixup);
    encode_instruction_provenance(hasher, &row.provenance);
    encode_control(hasher, &row.control);
}

fn encode_optional_internal_machine_fixup(
    hasher: &mut Sha256,
    fixup: Option<FunctionFragmentInternalMachineFixup>,
) {
    match fixup {
        None => hasher.update([0]),
        Some(fixup) => {
            hasher.update([1]);
            encode_internal_machine_fixup(hasher, fixup);
        }
    }
}

fn encode_internal_machine_fixup(hasher: &mut Sha256, fixup: FunctionFragmentInternalMachineFixup) {
    hasher.update([match fixup.kind {
        FunctionFragmentInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1 => 1,
        FunctionFragmentInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1 => 2,
    }]);
    hasher.update([match fixup.state {
        FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1 => 1,
    }]);
    hasher.update(fixup.callee.get().to_le_bytes());
    hasher.update(fixup.opcode_function_offset.to_le_bytes());
    hasher.update(fixup.patch_function_offset.to_le_bytes());
    hasher.update(fixup.reference_function_offset.to_le_bytes());
    hasher.update([fixup.patch_byte_width]);
    hasher.update(fixup.addend.to_le_bytes());
}

fn encode_target(hasher: &mut Sha256, target: NativeTarget) {
    hasher.update([match target.architecture {
        Architecture::Aarch64 => 0,
        Architecture::X86_64 => 1,
    }]);
    hasher.update([match target.object_format {
        ObjectFormat::Elf => 0,
        ObjectFormat::MachO => 1,
        ObjectFormat::Coff => 2,
    }]);
    hasher.update((target.pointer_size as u64).to_le_bytes());
    hasher.update((target.pointer_alignment as u64).to_le_bytes());
}

fn encode_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

fn encode_function_provenance(hasher: &mut Sha256, provenance: &TerminalPsiProvenance) {
    encode_semantic_ids(hasher, provenance.operations.iter().map(|id| id.get()));
    encode_semantic_ids(hasher, provenance.edges.iter().map(|id| id.get()));
}

fn encode_instruction_provenance(hasher: &mut Sha256, provenance: &SelectedInstructionProvenance) {
    encode_semantic_ids(hasher, provenance.operations.iter().map(|id| id.get()));
    encode_semantic_ids(hasher, provenance.values.iter().map(|id| id.get()));
    encode_semantic_ids(hasher, provenance.edges.iter().map(|id| id.get()));
    encode_semantic_ids(hasher, provenance.obligations.iter().map(|id| id.get()));
    encode_fuel(hasher, &provenance.fuel);
}

fn encode_semantic_ids(hasher: &mut Sha256, ids: impl ExactSizeIterator<Item = u64>) {
    hasher.update((ids.len() as u64).to_le_bytes());
    for id in ids {
        hasher.update(id.to_le_bytes());
    }
}

fn encode_fuel(hasher: &mut Sha256, fuel: &[FuelSettlement]) {
    hasher.update((fuel.len() as u64).to_le_bytes());
    for settlement in fuel {
        match settlement.site {
            PsiProvenance::Operation(operation) => {
                hasher.update([0]);
                hasher.update(operation.get().to_le_bytes());
            }
            PsiProvenance::Edge(edge) => {
                hasher.update([1]);
                hasher.update(edge.get().to_le_bytes());
            }
        }
        hasher.update(settlement.units.to_le_bytes());
    }
}

fn encode_control(hasher: &mut Sha256, control: &FunctionFragmentControlProvenance) {
    match control {
        FunctionFragmentControlProvenance::None => hasher.update([0]),
        FunctionFragmentControlProvenance::DirectInternalCall { callee } => {
            hasher.update([3]);
            hasher.update(callee.get().to_le_bytes());
        }
        FunctionFragmentControlProvenance::ConditionalBranch {
            predicate,
            when_taken,
            when_fallthrough,
        } => {
            hasher.update([1]);
            encode_branch_predicate(hasher, *predicate);
            encode_successor(hasher, when_taken);
            encode_successor(hasher, when_fallthrough);
        }
        FunctionFragmentControlProvenance::Return { psi_return_edge } => {
            hasher.update([2]);
            hasher.update(psi_return_edge.get().to_le_bytes());
        }
    }
}

fn encode_successor(hasher: &mut Sha256, successor: &FunctionFragmentSuccessorProvenance) {
    hasher.update(successor.psi_edge.get().to_le_bytes());
    hasher.update(successor.block.0.to_le_bytes());
    hasher.update(successor.source_target.get().to_le_bytes());
    hasher.update((successor.bindings.len() as u64).to_le_bytes());
    for binding in &successor.bindings {
        hasher.update(binding.parameter.get().to_le_bytes());
        hasher.update(binding.argument.get().to_le_bytes());
        match binding.scalar_type {
            ScalarType::Boolean => hasher.update([0]),
            ScalarType::Integer(integer) => {
                hasher.update([1]);
                hasher.update([match integer.carrier() {
                    IntegerCarrier::Fixed => 0,
                    IntegerCarrier::Address => 1,
                }]);
                hasher.update([match integer.sign() {
                    IntegerSign::Signed => 0,
                    IntegerSign::Unsigned => 1,
                }]);
                hasher.update(integer.bits().to_le_bytes());
            }
            ScalarType::IeeeFloat(format) => {
                hasher.update([2]);
                hasher.update([match format {
                    semantic_vocabulary::IeeeFloatFormat::Binary32 => 0,
                    semantic_vocabulary::IeeeFloatFormat::Binary64 => 1,
                }]);
            }
        }
    }
    encode_fuel(hasher, &successor.fuel);
}

fn encode_branch(hasher: &mut Sha256, branch: Option<&FunctionFragmentConditionalBranchEvidence>) {
    let Some(branch) = branch else {
        hasher.update([0]);
        return;
    };
    hasher.update([1]);
    encode_branch_predicate(hasher, branch.predicate);
    hasher.update(branch.source_block.0.to_le_bytes());
    hasher.update(branch.when_taken_edge.get().to_le_bytes());
    hasher.update(branch.when_taken_block.0.to_le_bytes());
    hasher.update(branch.when_taken_offset.to_le_bytes());
    hasher.update(branch.when_fallthrough_edge.get().to_le_bytes());
    hasher.update(branch.when_fallthrough_block.0.to_le_bytes());
    hasher.update(branch.when_fallthrough_offset.to_le_bytes());
    hasher.update(branch.byte_displacement.to_le_bytes());
    hasher.update((branch.decoded_register_reads.len() as u64).to_le_bytes());
    for read in &branch.decoded_register_reads {
        hasher.update(read.0.to_le_bytes());
    }
    encode_effects(hasher, &branch.decoded_effects);
}

fn encode_alternative(hasher: &mut Sha256, alternative: MachineAlternativeKey) {
    hasher.update([match alternative.family {
        MachineAlternativeFamily::CompareI64Zero => 0,
        MachineAlternativeFamily::MaterializeI64 => 1,
        MachineAlternativeFamily::CopyI64 => 2,
        MachineAlternativeFamily::ExactAddI64 => 3,
        MachineAlternativeFamily::ExactAddI64Immediate => 4,
        MachineAlternativeFamily::ExactSubtractI64 => 5,
        MachineAlternativeFamily::ConditionalBranchNonZero => 6,
        MachineAlternativeFamily::ReturnI64 => 7,
        MachineAlternativeFamily::ExactSubtractI64Immediate => 8,
        MachineAlternativeFamily::ReturnUnit => 9,
        MachineAlternativeFamily::CompareI64 => 10,
        MachineAlternativeFamily::ConditionalBranchU64LessThan => 11,
        MachineAlternativeFamily::ConditionalBranchI64LessThan => 12,
        MachineAlternativeFamily::CallI64 => 13,
    }]);
    hasher.update(alternative.variant.to_le_bytes());
}

fn encode_branch_predicate(
    hasher: &mut Sha256,
    predicate: FunctionFragmentConditionalBranchPredicate,
) {
    hasher.update([match predicate {
        FunctionFragmentConditionalBranchPredicate::NonZeroV1 => 0,
        FunctionFragmentConditionalBranchPredicate::U64LessThanV1 => 1,
        FunctionFragmentConditionalBranchPredicate::I64LessThanV1 => 2,
    }]);
}

fn encode_effects(hasher: &mut Sha256, effects: &MachineEncodedEffects) {
    encode_u16s(hasher, &effects.external_operand_reads);
    encode_u16s(hasher, &effects.external_operand_writes);
    encode_u16s(
        hasher,
        &effects
            .implicit_unit_uses
            .iter()
            .map(|id| id.0)
            .collect::<Vec<_>>(),
    );
    encode_u16s(
        hasher,
        &effects
            .implicit_unit_defs
            .iter()
            .map(|id| id.0)
            .collect::<Vec<_>>(),
    );
    encode_u16s(
        hasher,
        &effects
            .implicit_unit_clobbers
            .iter()
            .map(|id| id.0)
            .collect::<Vec<_>>(),
    );
    match effects.memory {
        MachineEncodedMemoryEffect::NoneV1 => hasher.update([0]),
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
        MachineEncodedTrapBehavior::MayArchitecturalFaultV1 => 1,
    }]);
    match effects.control {
        MachineEncodedControlEffect::FallThroughV1 => hasher.update([0]),
        MachineEncodedControlEffect::ConditionalRelativeBranchV1 => hasher.update([1]),
        MachineEncodedControlEffect::ReturnFromActivationStackV1 => hasher.update([2]),
        MachineEncodedControlEffect::ReturnIndirectRegisterV1 { target } => {
            hasher.update([3]);
            hasher.update(target.0.to_le_bytes());
        }
        MachineEncodedControlEffect::DirectRelativeCallV1 => hasher.update([4]),
    }
}

fn encode_u16s(hasher: &mut Sha256, values: &[u16]) {
    hasher.update((values.len() as u64).to_le_bytes());
    for value in values {
        hasher.update(value.to_le_bytes());
    }
}
