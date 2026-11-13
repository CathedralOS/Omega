/*
Canonical SHA-256 identity encoding for `FunctionFragmentEmissionPlan`
(`FunctionFragmentEmissionIdentity`, schema `omega.terminal.function-fragment-emission.v24`).

Every function fragment, block span, instruction span (including zero-byte elided/fused rows),
branch displacement, internal/foreign/image fixup, and semantic provenance roster is hashed in
canonical order so downstream frame application and text-section placement can authenticate
their input fragments without re-running fragment realization.
*/

use crate::identity_encoding::{
    encode_call_owner, encode_conditional_branch_arm, encode_invocation_site, encode_jump_arm,
    encode_views,
};

use super::{
    FunctionFragmentConditionalBranchPredicate, FunctionFragmentControlProvenance,
    FunctionFragmentEmissionPlan, FunctionFragmentInternalMachineFixup,
    FunctionFragmentInternalMachineFixupKind, FunctionFragmentInternalMachineFixupState,
    FunctionFragmentNormalizedForeignCallFixup, FunctionFragmentNormalizedForeignCallFixupKind,
    FunctionFragmentNormalizedForeignCallFixupState, FunctionFragmentSuccessorProvenance,
};
use optimization_core::FunctionFragmentEmissionIdentity;
use optimization_unit::{PsiProvenance, WorkSettlement};
use selected_instructions::SelectedInstructionProvenance;
use semantic_vocabulary::{IntegerCarrier, IntegerSign, ScalarType};
use target_operations::TerminalPsiProvenance;

const FRAGMENT_SCHEMA: &[u8] = b"omega.terminal.function-fragment-emission.v24";

pub(crate) fn function_fragment_emission_identity(
    plan: &FunctionFragmentEmissionPlan,
) -> FunctionFragmentEmissionIdentity {
    let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(FRAGMENT_SCHEMA);
    hasher.raw(&plan.psi.vocabulary_marker.get().to_le_bytes());
    hasher.raw(plan.psi.program_fingerprint.as_bytes());
    hasher.raw(&plan.work_schedule.marker().to_le_bytes());
    hasher.raw(&plan.selected.bytes());
    hasher.raw(&plan.target.identity_bytes());
    hasher.raw(&plan.entry.get().to_le_bytes());
    hasher.u64(plan.functions.len() as u64);
    for function in &plan.functions {
        hasher.raw(&function.machine.get().to_le_bytes());
        match function.attachment {
            None => hasher.byte(0),
            Some(attachment) => {
                hasher.byte(1);
                hasher.raw(&attachment.get().to_le_bytes());
            }
        }
        match function.nominal_cleanup_subject {
            None => hasher.byte(0),
            Some(attachment) => {
                hasher.byte(1);
                hasher.raw(&attachment.get().to_le_bytes());
            }
        }
        encode_function_provenance(&mut hasher, &function.provenance);
        hasher.raw(&function.byte_count.to_le_bytes());
        hasher.bytes(&function.bytes);
        hasher.u64(function.blocks.len() as u64);
        for block in &function.blocks {
            hasher.raw(&block.block.0.to_le_bytes());
            hasher.raw(&block.offset.to_le_bytes());
            hasher.raw(&block.byte_count.to_le_bytes());
            hasher.u64(block.instructions.len() as u64);
            for row in &block.instructions {
                hasher.raw(&row.instruction.0.to_le_bytes());
                selected_instructions::encode_machine_alternative_key_identity(
                    &mut hasher,
                    row.alternative,
                );
                hasher.raw(&row.offset.to_le_bytes());
                hasher.bytes(&row.bytes);
                encode_branch(&mut hasher, row.branch.as_deref());
                encode_optional_internal_machine_fixup(&mut hasher, row.internal_machine_fixup());
                encode_optional_normalized_foreign_call_fixup(
                    &mut hasher,
                    row.normalized_foreign_call_fixup(),
                );
                if let Some(fixup) = row.image_address_fixup() {
                    let mut identity = b"image-address-v2".to_vec();
                    fixup.reference.encode_identity(&mut identity);
                    fixup.fields.encode_identity(&mut identity);
                    hasher.raw(&identity);
                }

                encode_instruction_provenance(&mut hasher, &row.provenance);
                encode_control(&mut hasher, &row.control);
            }
        }
    }
    FunctionFragmentEmissionIdentity::from_canonical_bytes(&hasher.finish())
}

fn encode_optional_internal_machine_fixup(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    fixup: Option<FunctionFragmentInternalMachineFixup>,
) {
    match fixup {
        None => hasher.byte(0),
        Some(fixup) => {
            hasher.byte(1);
            encode_internal_machine_fixup(hasher, fixup);
        }
    }
}

fn encode_internal_machine_fixup(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    fixup: FunctionFragmentInternalMachineFixup,
) {
    hasher.byte(match fixup.kind {
        FunctionFragmentInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1 => 1,
        FunctionFragmentInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1 => 2,
    });
    hasher.byte(match fixup.state {
        FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1 => 1,
    });
    hasher.raw(&fixup.callee.get().to_le_bytes());
    hasher.raw(&fixup.opcode_function_offset.to_le_bytes());
    hasher.raw(&fixup.patch_function_offset.to_le_bytes());
    hasher.raw(&fixup.reference_function_offset.to_le_bytes());
    hasher.byte(fixup.patch_byte_width);
    hasher.raw(&fixup.addend.to_le_bytes());
}

fn encode_optional_normalized_foreign_call_fixup(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    fixup: Option<&FunctionFragmentNormalizedForeignCallFixup>,
) {
    match fixup {
        None => hasher.byte(0),
        Some(fixup) => {
            hasher.byte(1);
            hasher.byte(match fixup.kind {
                FunctionFragmentNormalizedForeignCallFixupKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1 => 1,
                FunctionFragmentNormalizedForeignCallFixupKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1 => 2,
            });
            hasher.byte(match fixup.state {
                FunctionFragmentNormalizedForeignCallFixupState::UnresolvedImportFieldV1 => 1,
            });
            hasher.raw(&fixup.boundary.get().to_le_bytes());
            hasher.raw(&fixup.ordinal.to_le_bytes());
            hasher.raw(&fixup.opcode_function_offset.to_le_bytes());
            hasher.raw(&fixup.patch_function_offset.to_le_bytes());
            hasher.raw(&fixup.reference_function_offset.to_le_bytes());
            hasher.byte(fixup.patch_byte_width);
            hasher.raw(&fixup.addend.to_le_bytes());
            hasher.u64(fixup.callbacks.len() as u64);
            for callback in &fixup.callbacks {
                crate::calls::callbacks::encode_callback_destination_identity(
                    hasher,
                    &callback.destination,
                );
                hasher.raw(&callback.code_function_offset.to_le_bytes());
                hasher.raw(&callback.code_byte_count.to_le_bytes());
                match callback.encoding {
                    super::FunctionFragmentCallbackMaterializationEncoding::X86_64Relative32 {
                        patch_function_offset,
                    } => {
                        hasher.byte(0);
                        hasher.raw(&patch_function_offset.to_le_bytes());
                    }
                    super::FunctionFragmentCallbackMaterializationEncoding::Aarch64PageAddress {
                        page_patch_function_offset,
                        page_offset_patch_function_offset,
                    } => {
                        hasher.byte(1);
                        hasher.raw(&page_patch_function_offset.to_le_bytes());
                        hasher.raw(&page_offset_patch_function_offset.to_le_bytes());
                    }
                }
            }
        }
    }
}

fn encode_function_provenance(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    provenance: &TerminalPsiProvenance,
) {
    encode_semantic_ids(hasher, provenance.operations.iter().map(|id| id.get()));
    encode_semantic_ids(hasher, provenance.edges.iter().map(|id| id.get()));
}

fn encode_instruction_provenance(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    provenance: &SelectedInstructionProvenance,
) {
    encode_semantic_ids(hasher, provenance.operations.iter().map(|id| id.get()));
    encode_semantic_ids(hasher, provenance.values.iter().map(|id| id.get()));
    encode_semantic_ids(hasher, provenance.edges.iter().map(|id| id.get()));
    encode_semantic_ids(hasher, provenance.obligations.iter().map(|id| id.get()));
    encode_work(hasher, &provenance.work);
}

fn encode_semantic_ids(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    ids: impl ExactSizeIterator<Item = u64>,
) {
    hasher.u64(ids.len() as u64);
    for id in ids {
        hasher.raw(&id.to_le_bytes());
    }
}

fn encode_work(hasher: &mut crate::digest_encoder::DomainDigestEncoder, work: &[WorkSettlement]) {
    hasher.u64(work.len() as u64);
    for settlement in work {
        match settlement.site {
            PsiProvenance::Operation(operation) => {
                hasher.byte(0);
                hasher.raw(&operation.get().to_le_bytes());
            }
            PsiProvenance::Edge(edge) => {
                hasher.byte(1);
                hasher.raw(&edge.get().to_le_bytes());
            }
        }
        hasher.raw(&settlement.units.to_le_bytes());
    }
}

fn encode_control(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    control: &FunctionFragmentControlProvenance,
) {
    match control {
        FunctionFragmentControlProvenance::OperationTrap { operation } => {
            hasher.byte(8);
            hasher.raw(&operation.get().to_le_bytes());
        }
        FunctionFragmentControlProvenance::Crash { psi_edge, cause } => {
            hasher.byte(7);
            hasher.raw(&psi_edge.get().to_le_bytes());
            hasher.byte(match cause {
                terminal_psi::CrashCause::Trap => 1,
                terminal_psi::CrashCause::Abort => 2,
            });
        }
        FunctionFragmentControlProvenance::Jump { successor } => {
            hasher.byte(4);
            encode_successor(hasher, successor);
        }
        FunctionFragmentControlProvenance::None => hasher.byte(0),
        FunctionFragmentControlProvenance::InternalCall { target, owner } => {
            match target {
                selected_instructions::SelectedCallTarget::Machine(machine) => {
                    hasher.byte(3);
                    hasher.raw(&machine.get().to_le_bytes());
                }
                selected_instructions::SelectedCallTarget::Dynamic { interface, slot } => {
                    hasher.byte(9);
                    hasher.raw(&interface.to_le_bytes());
                    hasher.raw(&slot.to_le_bytes());
                }
            }
            encode_call_owner(hasher, *owner);
        }
        FunctionFragmentControlProvenance::NormalizedForeignCall {
            invocation_site,
            boundary,
            ordinal,
        } => {
            hasher.byte(6);
            encode_invocation_site(hasher, *invocation_site);
            hasher.raw(&boundary.get().to_le_bytes());
            hasher.raw(&ordinal.to_le_bytes());
        }
        FunctionFragmentControlProvenance::ConditionalBranch {
            predicate,
            when_taken,
            when_fallthrough,
        } => {
            hasher.byte(1);
            encode_branch_predicate(hasher, *predicate);
            encode_successor(hasher, when_taken);
            encode_successor(hasher, when_fallthrough);
        }
        FunctionFragmentControlProvenance::Return { psi_return_edge } => {
            hasher.byte(2);
            hasher.raw(&psi_return_edge.get().to_le_bytes());
        }
        FunctionFragmentControlProvenance::HostedExitProcess { completion_edge } => {
            hasher.byte(5);
            hasher.raw(&completion_edge.get().to_le_bytes());
        }
    }
}

fn encode_successor(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    successor: &FunctionFragmentSuccessorProvenance,
) {
    hasher.byte(match successor.role {
        selected_instructions::SelectedSuccessorRole::Semantic => 0,
        selected_instructions::SelectedSuccessorRole::EdgeTransferContinuation => 1,
        selected_instructions::SelectedSuccessorRole::CaseDispatchContinuation => 2,
        selected_instructions::SelectedSuccessorRole::OperationContinuation { .. } => 3,
    });
    if let selected_instructions::SelectedSuccessorRole::OperationContinuation { operation } =
        successor.role
    {
        hasher.raw(&operation.get().to_le_bytes());
    }
    hasher.raw(
        &successor
            .psi_edge
            .map_or(0, |edge| edge.get())
            .to_le_bytes(),
    );
    hasher.raw(&successor.block.0.to_le_bytes());
    hasher.raw(&successor.source_target.get().to_le_bytes());
    hasher.u64(successor.bindings.len() as u64);
    for binding in &successor.bindings {
        hasher.raw(&binding.parameter.get().to_le_bytes());
        hasher.raw(&binding.argument.get().to_le_bytes());
        match binding.scalar_type {
            ScalarType::Callable(contract) => {
                hasher.byte(3);
                hasher.raw(&contract.get().to_le_bytes());
            }
            ScalarType::Boolean => hasher.byte(0),
            ScalarType::Integer(integer) => {
                hasher.byte(1);
                hasher.byte(match integer.carrier() {
                    IntegerCarrier::Fixed => 0,
                    IntegerCarrier::Address => 1,
                });
                hasher.byte(match integer.sign() {
                    IntegerSign::Signed => 0,
                    IntegerSign::Unsigned => 1,
                });
                hasher.raw(&integer.bits().to_le_bytes());
            }
            ScalarType::IeeeFloat(format) => {
                hasher.byte(2);
                hasher.byte(match format {
                    semantic_vocabulary::IeeeFloatFormat::Binary32 => 0,
                    semantic_vocabulary::IeeeFloatFormat::Binary64 => 1,
                });
            }
        }
    }
    encode_work(hasher, &successor.work);
}

fn encode_branch(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    branch: Option<&super::FunctionFragmentBranchEvidence>,
) {
    let Some(branch) = branch else {
        hasher.byte(0);
        return;
    };
    let branch = match branch {
        super::FunctionFragmentBranchEvidence::Conditional(branch) => branch,
        super::FunctionFragmentBranchEvidence::Jump(jump) => {
            encode_jump_arm(hasher, jump);
            selected_instructions::encode_machine_encoded_effects_identity(
                hasher,
                &jump.decoded_effects,
            );
            return;
        }
    };
    encode_conditional_branch_arm(hasher, branch);
    encode_views(hasher, &branch.decoded_register_reads);
    selected_instructions::encode_machine_encoded_effects_identity(hasher, &branch.decoded_effects);
}

fn encode_branch_predicate(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    predicate: FunctionFragmentConditionalBranchPredicate,
) {
    hasher.byte(predicate.rank());
}
