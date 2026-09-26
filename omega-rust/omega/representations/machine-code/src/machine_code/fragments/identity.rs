//! Canonical identity of the complete fragment program, including zero-code rows.

use super::{
    FunctionFragmentConditionalBranchPredicate, FunctionFragmentControlProvenance,
    FunctionFragmentEmissionPlan, FunctionFragmentInternalMachineFixup,
    FunctionFragmentInternalMachineFixupKind, FunctionFragmentInternalMachineFixupState,
    FunctionFragmentNormalizedForeignCallFixup, FunctionFragmentNormalizedForeignCallFixupKind,
    FunctionFragmentNormalizedForeignCallFixupState, FunctionFragmentSuccessorProvenance,
};
use optimization_core::FunctionFragmentEmissionIdentity;
use optimization_unit::{FuelSettlement, PsiProvenance};
use selected_instructions::SelectedInstructionProvenance;
use semantic_vocabulary::{IntegerCarrier, IntegerSign, ScalarType};
use sha2::{Digest, Sha256};
use target::{Architecture, NativeTarget, ObjectFormat};
use target_operations::TerminalPsiProvenance;

const FRAGMENT_SCHEMA: &[u8] = b"omega.terminal.function-fragment-emission.v15";

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
                selected_instructions::encode_machine_alternative_key_identity(
                    &mut hasher,
                    row.alternative,
                );
                hasher.update(row.offset.to_le_bytes());
                encode_bytes(&mut hasher, &row.bytes);
                encode_branch(&mut hasher, row.branch.as_deref());
                encode_optional_internal_machine_fixup(&mut hasher, row.internal_machine_fixup);
                encode_optional_normalized_foreign_call_fixup(
                    &mut hasher,
                    row.normalized_foreign_call_fixup,
                );
                encode_instruction_provenance(&mut hasher, &row.provenance);
                encode_control(&mut hasher, &row.control);
            }
        }
    }
    FunctionFragmentEmissionIdentity::from_canonical_bytes(&hasher.finalize())
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

fn encode_optional_normalized_foreign_call_fixup(
    hasher: &mut Sha256,
    fixup: Option<FunctionFragmentNormalizedForeignCallFixup>,
) {
    match fixup {
        None => hasher.update([0]),
        Some(fixup) => {
            hasher.update([1]);
            hasher.update([match fixup.kind {
                FunctionFragmentNormalizedForeignCallFixupKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1 => 1,
                FunctionFragmentNormalizedForeignCallFixupKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1 => 2,
            }]);
            hasher.update([match fixup.state {
                FunctionFragmentNormalizedForeignCallFixupState::UnresolvedImportFieldV1 => 1,
            }]);
            hasher.update(fixup.boundary.get().to_le_bytes());
            hasher.update(fixup.ordinal.to_le_bytes());
            hasher.update(fixup.opcode_function_offset.to_le_bytes());
            hasher.update(fixup.patch_function_offset.to_le_bytes());
            hasher.update(fixup.reference_function_offset.to_le_bytes());
            hasher.update([fixup.patch_byte_width]);
            hasher.update(fixup.addend.to_le_bytes());
        }
    }
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
        FunctionFragmentControlProvenance::Crash { psi_edge, cause } => {
            hasher.update([7]);
            hasher.update(psi_edge.get().to_le_bytes());
            hasher.update([match cause {
                terminal_psi::CrashCause::Trap => 1,
                terminal_psi::CrashCause::Abort => 2,
            }]);
        }
        FunctionFragmentControlProvenance::Jump { successor } => {
            hasher.update([4]);
            encode_successor(hasher, successor);
        }
        FunctionFragmentControlProvenance::None => hasher.update([0]),
        FunctionFragmentControlProvenance::DirectInternalCall { callee } => {
            hasher.update([3]);
            hasher.update(callee.get().to_le_bytes());
        }
        FunctionFragmentControlProvenance::NormalizedForeignCall { boundary, ordinal } => {
            hasher.update([6]);
            hasher.update(boundary.get().to_le_bytes());
            hasher.update(ordinal.to_le_bytes());
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
        FunctionFragmentControlProvenance::HostedExitProcess {
            nominal_return_edge,
        } => {
            hasher.update([5]);
            hasher.update(nominal_return_edge.get().to_le_bytes());
        }
    }
}

fn encode_successor(hasher: &mut Sha256, successor: &FunctionFragmentSuccessorProvenance) {
    hasher.update([match successor.role {
        selected_instructions::SelectedSuccessorRole::Semantic => 0,
        selected_instructions::SelectedSuccessorRole::EdgeTransferContinuation => 1,
        selected_instructions::SelectedSuccessorRole::CaseDispatchContinuation => 2,
    }]);
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

fn encode_branch(hasher: &mut Sha256, branch: Option<&super::FunctionFragmentBranchEvidence>) {
    let Some(branch) = branch else {
        hasher.update([0]);
        return;
    };
    let branch = match branch {
        super::FunctionFragmentBranchEvidence::Conditional(branch) => branch,
        super::FunctionFragmentBranchEvidence::Jump(jump) => {
            hasher.update([2]);
            hasher.update(jump.source_block.0.to_le_bytes());
            hasher.update(jump.target_edge.get().to_le_bytes());
            hasher.update(jump.target_block.0.to_le_bytes());
            hasher.update(jump.target_offset.to_le_bytes());
            hasher.update(jump.byte_displacement.to_le_bytes());
            selected_instructions::encode_machine_encoded_effects_identity(
                hasher,
                &jump.decoded_effects,
            );
            return;
        }
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
    selected_instructions::encode_machine_encoded_effects_identity(hasher, &branch.decoded_effects);
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
