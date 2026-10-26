//! Content coordinates of a target frame and its encoded protocol.
//!
//! These identify replay inputs; they do not establish ABI preservation.

register_homes::identity_newtype! { pub struct TargetFrameLayoutIdentity([u8; 32]); }

register_homes::identity_newtype! { pub struct TargetFrameProtocolEncodingIdentity([u8; 32]); }

use crate::{ReturnAddressFrameCustody, TargetFrameLayoutPlan, TargetFrameLayoutPolicy};

pub fn target_frame_layout_identity(plan: &TargetFrameLayoutPlan) -> TargetFrameLayoutIdentity {
    let mut hasher =
        crate::digest_encoder::DomainDigestEncoder::new(b"omega.target-frame-layout.v10");
    hasher.raw(&plan.post_allocation_machine.bytes());
    hasher.raw(&plan.callee_saved_requirements.bytes());
    hasher.raw(&plan.callee_save_storage.bytes());
    hasher.raw(&plan.register_environment.bytes());
    hasher.raw(&plan.physical_register_model.bytes());
    hasher.raw(&plan.target.identity_bytes());
    hasher.byte(plan.abi.abi_tag());
    hasher.byte(match plan.policy {
        TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1 => 0,
        TargetFrameLayoutPolicy::CanonicalSavedReturnAddressFrameV1 => 1,
    });
    encode_len(&mut hasher, plan.functions.len());
    for function in &plan.functions {
        hasher.raw(&function.machine.get().to_le_bytes());
        hasher.byte(u8::from(function.contains_call));
        hasher.raw(&function.stack_pointer.0.to_le_bytes());
        hasher.raw(&function.pre_call_stack_alignment.to_le_bytes());
        hasher.raw(&function.frame_size_bytes.to_le_bytes());
        hasher.raw(&function.red_zone_resident_bytes.to_le_bytes());
        hasher.raw(&function.abi_stack_alignment_bytes.to_le_bytes());
        hasher.raw(&function.outgoing_abi_area.byte_size.to_le_bytes());
        hasher.raw(&function.outgoing_abi_area.shadow_bytes.to_le_bytes());
        encode_len(&mut hasher, function.local_storage_slots.len());
        for slot in &function.local_storage_slots {
            let mut identity = Vec::new();
            slot.id.encode_identity(&mut identity);
            hasher.raw(&identity);
            hasher.raw(&slot.frame_offset_bytes.to_le_bytes());
            hasher.raw(&slot.size_bytes.to_le_bytes());
            hasher.raw(&slot.alignment_bytes.to_le_bytes());
        }
        encode_len(&mut hasher, function.stable_address_loans.len());
        for slot in &function.stable_address_loans {
            let mut identity = Vec::new();
            slot.encode_identity(&mut identity);
            hasher.raw(&identity);
        }
        encode_len(&mut hasher, function.callee_save_slots.len());
        for slot in &function.callee_save_slots {
            hasher.raw(&slot.abstract_slot.0.to_le_bytes());
            hasher.raw(&slot.storage_view.0.to_le_bytes());
            hasher.raw(&slot.frame_offset_bytes.to_le_bytes());
            hasher.raw(&slot.size_bytes.to_le_bytes());
            hasher.raw(&slot.alignment_bytes.to_le_bytes());
        }
        encode_return_address(&mut hasher, function.return_address);
        hasher.raw(&function.stack_probe.interval_bytes.to_le_bytes());
        hasher.raw(&function.stack_probe.touches.to_le_bytes());
        encode_len(&mut hasher, function.unwind.restores.len());
        for restore in &function.unwind.restores {
            hasher.raw(&restore.view.0.to_le_bytes());
            hasher.raw(&restore.frame_offset_bytes.to_le_bytes());
            hasher.raw(&restore.size_bytes.to_le_bytes());
        }
        hasher.raw(&function.unwind.released_bytes.to_le_bytes());
        encode_return_address(&mut hasher, function.unwind.return_address);
    }
    TargetFrameLayoutIdentity::from_bytes(hasher.finish())
}

fn encode_return_address(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    custody: ReturnAddressFrameCustody,
) {
    match custody {
        ReturnAddressFrameCustody::CallerActivationStack {
            post_prologue_offset_bytes,
            size_bytes,
        } => {
            hasher.byte(0);
            hasher.raw(&post_prologue_offset_bytes.to_le_bytes());
            hasher.raw(&size_bytes.to_le_bytes());
        }
        ReturnAddressFrameCustody::LiveLinkRegister { view } => {
            hasher.byte(1);
            hasher.raw(&view.0.to_le_bytes());
        }
        ReturnAddressFrameCustody::SavedLinkRegister {
            view,
            frame_offset_bytes,
            size_bytes,
        } => {
            hasher.byte(2);
            hasher.raw(&view.0.to_le_bytes());
            hasher.raw(&frame_offset_bytes.to_le_bytes());
            hasher.raw(&size_bytes.to_le_bytes());
        }
    }
}

fn encode_len(hasher: &mut crate::digest_encoder::DomainDigestEncoder, value: usize) {
    hasher.u64(value as u64);
}
