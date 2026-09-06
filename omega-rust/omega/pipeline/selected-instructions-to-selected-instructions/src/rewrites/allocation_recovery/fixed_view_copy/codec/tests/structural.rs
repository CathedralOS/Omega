//! Optimizer module role: stage group. Structural-payload and envelope custody.

use calling_conventions::{CallPlan, CallingPolicy, EntryControl, MachineRegister, RegisterSet};
use optimization_unit::EffectLink;
use register_model::{RegisterConstraintFamily, RegisterConstraintKey, RegisterUnitId};
use selected_instructions::{
    SelectedBlockId, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMicrosoftX64OwnedIndirectPairLayout,
    SelectedStructuralUnitAbi, SelectedStructuralUnitAbiRecipe,
    SelectedStructuralUnitCallInstruction, SelectedStructuralUnitCallSource,
    SelectedStructuralUnitFunction, SelectedStructuralUnitIndirectBinding,
    SelectedStructuralUnitReturn,
};
use semantic_vocabulary::{BlockId, EdgeId, MachineId, ObligationId, OperationId};
use sha2::{Digest, Sha256};
use terminal_psi::{CrashCause, CrashRouteBucket, CrashRouteGuard};

use crate::{FixedViewCopyDecodeError, FixedViewCopyPlan, FixedViewCopyPolicy};

use super::{
    super::{copy::decode_copy, primitives::Cursor},
    plan,
};

fn call_plan(clobbers: &[MachineRegister], shadow_bytes: u16) -> CallPlan {
    CallPlan {
        policy: CallingPolicy::MicrosoftX64,
        parameters: Vec::new(),
        result: None,
        callback_materializations: Vec::new(),
        ordinary_clobbers: RegisterSet::new(clobbers.iter().copied()),
        stack_alignment: 16,
        shadow_bytes,
        entry_control: EntryControl::CallReturn,
    }
}

fn layout() -> SelectedMicrosoftX64OwnedIndirectPairLayout {
    SelectedMicrosoftX64OwnedIndirectPairLayout {
        shadow_byte_count: 32,
        outgoing_frame_byte_count: 72,
        pre_call_stack_alignment: 16,
        bindings: [
            SelectedStructuralUnitIndirectBinding {
                parameter_index: 0,
                pointer: MachineRegister::X86Rcx,
                copy_stack_byte_offset: 32,
                byte_count: 16,
                alignment: 8,
            },
            SelectedStructuralUnitIndirectBinding {
                parameter_index: 1,
                pointer: MachineRegister::X86Rdx,
                copy_stack_byte_offset: 48,
                byte_count: 16,
                alignment: 8,
            },
        ],
    }
}

fn structural_function() -> SelectedStructuralUnitFunction {
    let return_constraint = RegisterConstraintKey {
        family: RegisterConstraintFamily::Return,
        variant: 1,
    };
    SelectedStructuralUnitFunction {
        machine: MachineId::new(41).unwrap(),
        attachment: None,
        provenance: Default::default(),
        structural_types: Vec::new(),
        abi: SelectedStructuralUnitAbi {
            recipe: SelectedStructuralUnitAbiRecipe::MicrosoftX64OwnedIndirectPairV1,
            call_plan: call_plan(&[MachineRegister::X86Rbx], 32),
            parameters: Vec::new(),
            layout: layout(),
        },
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        entry_block: SelectedBlockId(0),
        source_entry_block: BlockId::new(41).unwrap(),
        boundary_settlements: Vec::new(),
        call: Some(SelectedStructuralUnitCallInstruction {
            id: SelectedInstructionId(0),
            source: SelectedStructuralUnitCallSource::AuthoredCallUnit,
            operation: OperationId::new(41).unwrap(),
            callee: MachineId::new(42).unwrap(),
            caller_call_plan: call_plan(
                &[MachineRegister::X86Rax, MachineRegister::X86Rcx],
                0x4567,
            ),
            callee_call_plan: call_plan(&[MachineRegister::X86Rdx], 0x6789),
            arguments: Vec::new(),
            claim_transfers: Vec::new(),
            layout: layout(),
            constraint: RegisterConstraintKey {
                family: RegisterConstraintFamily::Call,
                variant: 1,
            },
            implicit_uses: vec![RegisterUnitId(1)],
            implicit_defs: vec![RegisterUnitId(2)],
            clobbers: vec![RegisterUnitId(3)],
            provenance: Default::default(),
            effect: EffectLink {
                input: 10,
                output: 11,
            },
            requirement_obligations: vec![ObligationId::new(43).unwrap()],
            crash_continuations: vec![CrashRouteBucket {
                cause: CrashCause::Trap,
                alternatives: vec![CrashRouteGuard::Truth],
            }],
            ownership: Vec::new(),
        }),
        terminator: SelectedStructuralUnitReturn {
            instruction: SelectedInstruction {
                id: SelectedInstructionId(1),
                kind: SelectedInstructionKind::ReturnUnit,
                constraint: return_constraint,
                operands: Vec::new(),
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
                provenance: SelectedInstructionProvenance::default(),
            },
            psi_return_edge: EdgeId::new(41).unwrap(),
            effect: EffectLink {
                input: 11,
                output: 12,
            },
            ownership: Vec::new(),
        },
    }
}

fn transformed_identity_offset(encoded: &[u8]) -> usize {
    let mut cursor = Cursor::new(encoded);
    cursor.take(44 + (5 * 32) + 1 + 40 + 40).unwrap();
    let copy_count = cursor.length().unwrap();
    for _ in 0..copy_count {
        decode_copy(&mut cursor).unwrap();
    }
    cursor.offset
}

fn selected_payload_offset(encoded: &[u8]) -> usize {
    let mut cursor = Cursor::new(encoded);
    cursor
        .take(transformed_identity_offset(encoded) + 32)
        .unwrap();
    super::super::evidence::decode(&mut cursor).unwrap();
    cursor.offset
}

#[test]
fn artifact_v12_round_trips_structural_functions_call_plans_and_semantic_call_rows() {
    let mut plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    std::sync::Arc::make_mut(&mut plan.transformed)
        .structural_unit_functions
        .push(structural_function());
    assert_eq!(FixedViewCopyPlan::decode(&plan.encode()).unwrap(), plan);
}

#[test]
fn artifact_v5_decodes_with_empty_semantic_call_rows() {
    let mut plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    std::sync::Arc::make_mut(&mut plan.transformed)
        .structural_unit_functions
        .push(structural_function());

    let encoded = super::super::encode_v5(&plan);
    let mut expected = plan;
    let call = std::sync::Arc::make_mut(&mut expected.transformed).structural_unit_functions[0]
        .call
        .as_mut()
        .unwrap();
    call.requirement_obligations.clear();
    call.crash_continuations.clear();
    assert_eq!(FixedViewCopyPlan::decode(&encoded).unwrap(), expected);
}

#[test]
fn artifact_v12_payload_digest_and_outer_envelope_close_call_plan_blind_spots() {
    let mut plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    std::sync::Arc::make_mut(&mut plan.transformed)
        .structural_unit_functions
        .push(structural_function());
    let encoded = plan.encode();
    let digest_offset = selected_payload_offset(&encoded);

    let mut digest_tamper = encoded.clone();
    digest_tamper[digest_offset] ^= 1;
    assert_eq!(
        FixedViewCopyPlan::decode(&digest_tamper),
        Err(FixedViewCopyDecodeError::TransformedPayloadMismatch)
    );

    let payload_offset = digest_offset + 32 + 8;
    let marker = 0x4567_u16.to_le_bytes();
    let matches = encoded[payload_offset..]
        .windows(marker.len())
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == marker).then_some(payload_offset + offset))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "caller call-plan marker must be unique");
    let mut payload_and_digest_tamper = encoded;
    payload_and_digest_tamper[matches[0]..matches[0] + 2]
        .copy_from_slice(&0x4568_u16.to_le_bytes());
    let payload_digest =
        <[u8; 32]>::from(Sha256::digest(&payload_and_digest_tamper[payload_offset..]));
    payload_and_digest_tamper[digest_offset..digest_offset + 32].copy_from_slice(&payload_digest);
    assert_eq!(
        FixedViewCopyPlan::decode(&payload_and_digest_tamper),
        Err(FixedViewCopyDecodeError::IdentityMismatch)
    );

    let clobber_marker = [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0x10, 0, 0x67, 0x45, 1];
    let matches = plan
        .encode()
        .windows(clobber_marker.len())
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == clobber_marker).then_some(offset))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "caller clobber marker must be unique");
    let mut noncanonical = plan.encode();
    noncanonical[matches[0] + 8..matches[0] + 12].copy_from_slice(&[1, 0, 0, 0]);
    let payload_digest = <[u8; 32]>::from(Sha256::digest(&noncanonical[payload_offset..]));
    noncanonical[digest_offset..digest_offset + 32].copy_from_slice(&payload_digest);
    assert_eq!(
        FixedViewCopyPlan::decode(&noncanonical),
        Err(FixedViewCopyDecodeError::TransformedPayloadMismatch)
    );
}
