//! Ordinary structural metadata and envelope custody.
use super::{
    super::{copy::decode_copy, primitives::Cursor},
    plan,
};
use crate::{FixedViewCopyDecodeError, FixedViewCopyPlan, FixedViewCopyPolicy};
use calling_conventions::{CallPlan, CallingPolicy, EntryControl, MachineRegister, RegisterSet};
use legalized_operations::{
    LegalizedCallUnitSource, LegalizedScalarCall, LegalizedStructuralContract,
};
use optimization_unit::EffectLink;
use selected_instructions::{
    OutgoingArgumentSlotId, SelectedCallContract, SelectedFunction, SelectedInstructionId,
    SelectedMemoryAccess, SelectedMemoryAccessRole, SelectedOutgoingArgumentSlot,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{MachineId, ObligationId, OperationId, PlaceId};
use sha2::{Digest, Sha256};
use terminal_psi::{CrashCause, CrashRouteBucket, CrashRouteGuard};
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
fn structural_function() -> SelectedFunction {
    let mut function = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1)
        .transformed
        .functions[0]
        .clone();
    function.machine = MachineId::new(41).unwrap();
    let mut pointer = function.virtual_registers[0].clone();
    pointer.id = VirtualRegisterId(function.virtual_registers.len() as u32);
    pointer.origin = VirtualRegisterOrigin::StructuralParameter {
        place: PlaceId::new(1).unwrap(),
        parameter_index: 1,
    };
    pointer.definition_site = None;
    function.virtual_registers.push(pointer.clone());
    pointer.id = VirtualRegisterId(function.virtual_registers.len() as u32);
    pointer.origin = VirtualRegisterOrigin::AbiTransport {
        instruction: SelectedInstructionId(0),
        place: PlaceId::new(1).unwrap(),
        byte_offset: 8,
    };
    function.virtual_registers.push(pointer);
    function.structural = Some(LegalizedStructuralContract {
        structural_types: Vec::new(),
        parameters: Vec::new(),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    function.calls = vec![SelectedCallContract {
        instruction: SelectedInstructionId(0),
        operation: OperationId::new(41).unwrap(),
        call: LegalizedScalarCall {
            source: LegalizedCallUnitSource::AuthoredCallUnit,
            callee: MachineId::new(42).unwrap(),
            call_plan: call_plan(&[MachineRegister::X86Rax, MachineRegister::X86Rcx], 0x4567),
            arguments: Vec::new(),
            result_placement: None,
            claim_transfers: Vec::new(),
            requirement_obligations: vec![ObligationId::new(43).unwrap()],
            crash_continuations: vec![CrashRouteBucket {
                cause: CrashCause::Trap,
                alternatives: vec![CrashRouteGuard::Truth],
            }],
        },
        effect: EffectLink {
            input: 10,
            output: 11,
        },
        ownership: Vec::new(),
    }];
    let slot = OutgoingArgumentSlotId {
        operation: OperationId::new(41).unwrap(),
        argument_index: 0,
    };
    function.outgoing_arguments = vec![SelectedOutgoingArgumentSlot {
        id: slot,
        byte_size: 16,
        alignment: 8,
        abi_stack_byte_offset: 32,
    }];
    function.memory_accesses = vec![SelectedMemoryAccess {
        instruction: SelectedInstructionId(0),
        operation: slot.operation,
        place: PlaceId::new(1).unwrap(),
        byte_offset: 8,
        byte_count: 8,
        role: SelectedMemoryAccessRole::WriteOutgoing { slot },
    }];
    function
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
fn artifact_v15_round_trips_structural_functions_call_plans_and_semantic_call_rows() {
    let mut plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    std::sync::Arc::make_mut(&mut plan.transformed)
        .functions
        .push(structural_function());
    assert_eq!(FixedViewCopyPlan::decode(&plan.encode()).unwrap(), plan);
}

#[test]
fn activation_local_roster_and_memory_roles_round_trip() {
    let mut plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let mut function = structural_function();
    let slot = selected_instructions::LocalStorageSlotId::Structural {
        operation: OperationId::new(53).unwrap(),
        place: PlaceId::new(59).unwrap(),
    };
    function
        .local_storage_slots
        .push(selected_instructions::SelectedLocalStorageSlot {
            id: slot,
            byte_size: 24,
            alignment: 8,
        });
    for role in [
        SelectedMemoryAccessRole::WriteLocal { slot },
        SelectedMemoryAccessRole::AddressLocal { slot },
    ] {
        function.memory_accesses.push(SelectedMemoryAccess {
            instruction: SelectedInstructionId(1),
            operation: slot.operation().expect("source-backed local slot"),
            place: slot.structural_place().unwrap(),
            byte_offset: 0,
            byte_count: 8,
            role,
        });
    }
    std::sync::Arc::make_mut(&mut plan.transformed)
        .functions
        .push(function);
    assert_eq!(FixedViewCopyPlan::decode(&plan.encode()).unwrap(), plan);
}

#[test]
fn stale_header_rejects_current_structural_payload() {
    let mut plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    std::sync::Arc::make_mut(&mut plan.transformed)
        .functions
        .push(structural_function());

    let encoded = super::with_stale_version(&plan, 5);
    assert_eq!(
        FixedViewCopyPlan::decode(&encoded),
        Err(FixedViewCopyDecodeError::UnsupportedVersion(5))
    );
}

#[test]
fn artifact_v15_payload_digest_and_outer_envelope_close_call_plan_blind_spots() {
    let mut plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    std::sync::Arc::make_mut(&mut plan.transformed)
        .functions
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
    assert_eq!(matches.len(), 1, "call-plan marker must be unique");
    let mut payload_and_digest_tamper = encoded;
    payload_and_digest_tamper[matches[0]..matches[0] + 2]
        .copy_from_slice(&0x4568_u16.to_le_bytes());
    let payload_digest =
        <[u8; 32]>::from(Sha256::digest(&payload_and_digest_tamper[payload_offset..]));
    payload_and_digest_tamper[digest_offset..digest_offset + 32].copy_from_slice(&payload_digest);
    assert_eq!(
        FixedViewCopyPlan::decode(&payload_and_digest_tamper),
        Err(FixedViewCopyDecodeError::TransformedIdentityMismatch)
    );

    let clobber_marker = [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0x10, 0, 0x67, 0x45, 1];
    let matches = plan
        .encode()
        .windows(clobber_marker.len())
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == clobber_marker).then_some(offset))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "call clobber marker must be unique");
    let mut noncanonical = plan.encode();
    noncanonical[matches[0] + 8..matches[0] + 12].copy_from_slice(&[1, 0, 0, 0]);
    let payload_digest = <[u8; 32]>::from(Sha256::digest(&noncanonical[payload_offset..]));
    noncanonical[digest_offset..digest_offset + 32].copy_from_slice(&payload_digest);
    assert_eq!(
        FixedViewCopyPlan::decode(&noncanonical),
        Err(FixedViewCopyDecodeError::TransformedPayloadMismatch)
    );
}

#[test]
fn compiler_spill_origin_and_local_slot_round_trip_without_source_authority() {
    let mut source = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let function = &mut std::sync::Arc::make_mut(&mut source.transformed).functions[0];
    let slot = selected_instructions::LocalStorageSlotId::Spill {
        register: VirtualRegisterId(0),
    };
    assert_eq!(slot.operation(), None);
    assert_eq!(slot.structural_place(), None);
    function
        .local_storage_slots
        .push(selected_instructions::SelectedLocalStorageSlot {
            id: slot,
            byte_size: 8,
            alignment: 8,
        });
    function.virtual_registers[0].origin = VirtualRegisterOrigin::SpillAddress {
        instruction: SelectedInstructionId(0),
        register: VirtualRegisterId(0),
    };
    function.virtual_registers[0].definition_site = None;
    assert_eq!(FixedViewCopyPlan::decode(&source.encode()).unwrap(), source);
}
