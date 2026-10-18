//! Canonical identity of the complete fragment program, including zero-code rows.

use crate::machine_code::identity_encoding::{encode_call_owner, encode_invocation_site};

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
use target_operations::TerminalPsiProvenance;

const FRAGMENT_SCHEMA: &[u8] = b"omega.terminal.function-fragment-emission.v18";

pub(crate) fn function_fragment_emission_identity(
    plan: &FunctionFragmentEmissionPlan,
) -> FunctionFragmentEmissionIdentity {
    let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(FRAGMENT_SCHEMA);
    hasher.raw(&plan.psi.vocabulary_marker.get().to_le_bytes());
    hasher.raw(plan.psi.program_fingerprint.as_bytes());
    hasher.raw(&plan.fuel_schedule.marker().to_le_bytes());
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
        encode_bytes(&mut hasher, &function.bytes);
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
                encode_bytes(&mut hasher, &row.bytes);
                encode_branch(&mut hasher, row.branch.as_deref());
                encode_optional_internal_machine_fixup(&mut hasher, row.internal_machine_fixup);
                encode_optional_normalized_foreign_call_fixup(
                    &mut hasher,
                    row.normalized_foreign_call_fixup.as_ref(),
                );
                // A row without an image address keeps its earlier identity.
                if let Some(fixup) = row.image_address_fixup {
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
                crate::machine_code::calls::callbacks::encode_callback_destination_identity(
                    hasher,
                    callback.destination,
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

fn encode_bytes(hasher: &mut crate::digest_encoder::DomainDigestEncoder, bytes: &[u8]) {
    hasher.u64(bytes.len() as u64);
    hasher.raw(&bytes);
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
    encode_fuel(hasher, &provenance.fuel);
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

fn encode_fuel(hasher: &mut crate::digest_encoder::DomainDigestEncoder, fuel: &[FuelSettlement]) {
    hasher.u64(fuel.len() as u64);
    for settlement in fuel {
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
    });
    hasher.raw(&successor.psi_edge.get().to_le_bytes());
    hasher.raw(&successor.block.0.to_le_bytes());
    hasher.raw(&successor.source_target.get().to_le_bytes());
    hasher.u64(successor.bindings.len() as u64);
    for binding in &successor.bindings {
        hasher.raw(&binding.parameter.get().to_le_bytes());
        hasher.raw(&binding.argument.get().to_le_bytes());
        match binding.scalar_type {
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
    encode_fuel(hasher, &successor.fuel);
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
            hasher.byte(2);
            hasher.raw(&jump.source_block.0.to_le_bytes());
            hasher.raw(&jump.target_edge.get().to_le_bytes());
            hasher.raw(&jump.target_block.0.to_le_bytes());
            hasher.raw(&jump.target_offset.to_le_bytes());
            hasher.raw(&jump.byte_displacement.to_le_bytes());
            selected_instructions::encode_machine_encoded_effects_identity(
                hasher,
                &jump.decoded_effects,
            );
            return;
        }
    };
    hasher.byte(1);
    encode_branch_predicate(hasher, branch.predicate);
    hasher.raw(&branch.source_block.0.to_le_bytes());
    hasher.raw(&branch.when_taken_edge.get().to_le_bytes());
    hasher.raw(&branch.when_taken_block.0.to_le_bytes());
    hasher.raw(&branch.when_taken_offset.to_le_bytes());
    hasher.raw(&branch.when_fallthrough_edge.get().to_le_bytes());
    hasher.raw(&branch.when_fallthrough_block.0.to_le_bytes());
    hasher.raw(&branch.when_fallthrough_offset.to_le_bytes());
    hasher.raw(&branch.byte_displacement.to_le_bytes());
    hasher.u64(branch.decoded_register_reads.len() as u64);
    for read in &branch.decoded_register_reads {
        hasher.raw(&read.0.to_le_bytes());
    }
    selected_instructions::encode_machine_encoded_effects_identity(hasher, &branch.decoded_effects);
}

fn encode_branch_predicate(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    predicate: FunctionFragmentConditionalBranchPredicate,
) {
    hasher.byte(predicate.rank());
}
