use sha2::{Digest, Sha256};

use crate::frame_layout::save_storage::{
    FrameAbiPreservationConvention, encode_callee_saved_modification_witness_identity,
};

use super::{
    NonAuthoritativeCalleeSaveStorageIdentity, NonAuthoritativeCalleeSaveStoragePlan,
    NonAuthoritativeCalleeSaveStoragePolicy,
};

pub fn non_authoritative_callee_save_storage_identity(
    plan: &NonAuthoritativeCalleeSaveStoragePlan,
) -> NonAuthoritativeCalleeSaveStorageIdentity {
    let mut hasher = Sha256::new();
    hasher.update(b"omega.non-authoritative-callee-save-storage.v2");
    hasher.update(plan.callee_saved_requirements.bytes());
    hasher.update(plan.register_environment.bytes());
    hasher.update(plan.physical_register_model.bytes());
    hasher.update(plan.preservation_storage_catalog.bytes());
    target(&mut hasher, plan.target);
    hasher.update([abi_tag(plan.abi)]);
    units(&mut hasher, &plan.callee_saved_units);
    hasher.update([match plan.policy {
        NonAuthoritativeCalleeSaveStoragePolicy::CanonicalTargetPreservationGroupsV1 => 0,
    }]);
    hasher.update(plan.budget.encode());
    hasher.update(plan.usage.encode());
    length(&mut hasher, plan.functions.len());
    for function in &plan.functions {
        hasher.update(function.machine.get().to_le_bytes());
        hasher.update(function.abstract_area_bytes.to_le_bytes());
        hasher.update(function.abstract_area_alignment.to_le_bytes());
        length(&mut hasher, function.slots.len());
        for slot in &function.slots {
            hasher.update(slot.id.0.to_le_bytes());
            hasher.update(slot.storage_group.0.to_le_bytes());
            hasher.update(slot.storage_view.0.to_le_bytes());
            units(&mut hasher, &slot.preserved_units);
            length(&mut hasher, slot.modified_units.len());
            for requirement in &slot.modified_units {
                hasher.update(requirement.unit.0.to_le_bytes());
                length(&mut hasher, requirement.witnesses.len());
                for witness in &requirement.witnesses {
                    encode_callee_saved_modification_witness_identity(&mut hasher, *witness);
                }
            }
            hasher.update(slot.abstract_offset_bytes.to_le_bytes());
            hasher.update(slot.size_bytes.to_le_bytes());
            hasher.update(slot.alignment_bytes.to_le_bytes());
        }
    }
    NonAuthoritativeCalleeSaveStorageIdentity::from_bytes(hasher.finalize().into())
}

fn target(hasher: &mut Sha256, target: target::NativeTarget) {
    hasher.update([match target.architecture {
        target::Architecture::Aarch64 => 0,
        target::Architecture::X86_64 => 1,
    }]);
    hasher.update([match target.object_format {
        target::ObjectFormat::Elf => 0,
        target::ObjectFormat::MachO => 1,
        target::ObjectFormat::Coff => 2,
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

fn units(hasher: &mut Sha256, values: &[register_model::RegisterUnitId]) {
    length(hasher, values.len());
    for value in values {
        hasher.update(value.0.to_le_bytes());
    }
}

fn length(hasher: &mut Sha256, value: usize) {
    hasher.update((value as u64).to_le_bytes());
}
