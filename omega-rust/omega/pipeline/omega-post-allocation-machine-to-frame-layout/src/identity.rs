use sha2::{Digest, Sha256};

use crate::{AllocatedCalleeSavedFunctionKind, FrameAbiPreservationConvention};

use super::{
    ReturnAddressFrameCustody, TargetFrameLayoutIdentity, TargetFrameLayoutPlan,
    TargetFrameLayoutPolicy,
};

pub fn target_frame_layout_identity(plan: &TargetFrameLayoutPlan) -> TargetFrameLayoutIdentity {
    let mut hasher = Sha256::new();
    hasher.update(b"omega.target-frame-layout.v1");
    hasher.update(plan.post_allocation_machine.bytes());
    hasher.update(plan.callee_saved_requirements.bytes());
    hasher.update(plan.callee_save_storage.bytes());
    hasher.update(plan.register_environment.bytes());
    hasher.update(plan.physical_register_model.bytes());
    encode_target(&mut hasher, plan.target);
    hasher.update([abi_tag(plan.abi)]);
    hasher.update([match plan.policy {
        TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1 => 0,
        TargetFrameLayoutPolicy::CanonicalSavedReturnAddressFrameV1 => 1,
    }]);
    encode_len(&mut hasher, plan.functions.len());
    for function in &plan.functions {
        hasher.update(function.machine.get().to_le_bytes());
        hasher.update([match function.kind {
            AllocatedCalleeSavedFunctionKind::Ordinary => 0,
            AllocatedCalleeSavedFunctionKind::StructuralUnit => 1,
        }]);
        hasher.update([u8::from(function.contains_call)]);
        hasher.update(function.stack_pointer.0.to_le_bytes());
        hasher.update(function.pre_call_stack_alignment.to_le_bytes());
        hasher.update(function.frame_size_bytes.to_le_bytes());
        hasher.update(function.abi_stack_alignment_bytes.to_le_bytes());
        encode_len(&mut hasher, function.callee_save_slots.len());
        for slot in &function.callee_save_slots {
            hasher.update(slot.abstract_slot.0.to_le_bytes());
            hasher.update(slot.storage_view.0.to_le_bytes());
            hasher.update(slot.frame_offset_bytes.to_le_bytes());
            hasher.update(slot.size_bytes.to_le_bytes());
            hasher.update(slot.alignment_bytes.to_le_bytes());
        }
        match function.return_address {
            ReturnAddressFrameCustody::CallerActivationStack {
                post_prologue_offset_bytes,
                size_bytes,
            } => {
                hasher.update([0]);
                hasher.update(post_prologue_offset_bytes.to_le_bytes());
                hasher.update(size_bytes.to_le_bytes());
            }
            ReturnAddressFrameCustody::LiveLinkRegister { view } => {
                hasher.update([1]);
                hasher.update(view.0.to_le_bytes());
            }
            ReturnAddressFrameCustody::SavedLinkRegister {
                view,
                frame_offset_bytes,
                size_bytes,
            } => {
                hasher.update([2]);
                hasher.update(view.0.to_le_bytes());
                hasher.update(frame_offset_bytes.to_le_bytes());
                hasher.update(size_bytes.to_le_bytes());
            }
        }
    }
    TargetFrameLayoutIdentity::from_bytes(hasher.finalize().into())
}

fn encode_target(hasher: &mut Sha256, target: omega_target::NativeTarget) {
    hasher.update([match target.architecture {
        omega_target::Architecture::Aarch64 => 0,
        omega_target::Architecture::X86_64 => 1,
    }]);
    hasher.update([match target.object_format {
        omega_target::ObjectFormat::Elf => 0,
        omega_target::ObjectFormat::MachO => 1,
        omega_target::ObjectFormat::Coff => 2,
    }]);
    hasher.update((target.pointer_size as u64).to_le_bytes());
    hasher.update((target.pointer_alignment as u64).to_le_bytes());
}

fn abi_tag(abi: FrameAbiPreservationConvention) -> u8 {
    match abi {
        FrameAbiPreservationConvention::SystemVAMD64 => 0,
        FrameAbiPreservationConvention::MicrosoftX64 => 1,
        FrameAbiPreservationConvention::Aapcs64 => 2,
        FrameAbiPreservationConvention::DarwinAapcs64 => 3,
    }
}

fn encode_len(hasher: &mut Sha256, value: usize) {
    hasher.update((value as u64).to_le_bytes());
}
