use sha2::{Digest, Sha256};

use crate::frame_layout::save_storage::encode_callee_saved_modification_witness_identity;

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
    plan.target.encode_identity(&mut hasher);
    hasher.update([plan.abi.abi_tag()]);
    register_model::sha256_register_units(&mut hasher, &plan.callee_saved_units);
    hasher.update([match plan.policy {
        NonAuthoritativeCalleeSaveStoragePolicy::CanonicalTargetPreservationGroupsV1 => 0,
    }]);
    hasher.update(plan.budget.encode());
    hasher.update(plan.usage.encode());
    register_model::sha256_length(&mut hasher, plan.functions.len());
    for function in &plan.functions {
        hasher.update(function.machine.get().to_le_bytes());
        hasher.update(function.abstract_area_bytes.to_le_bytes());
        hasher.update(function.abstract_area_alignment.to_le_bytes());
        register_model::sha256_length(&mut hasher, function.slots.len());
        for slot in &function.slots {
            hasher.update(slot.id.0.to_le_bytes());
            hasher.update(slot.storage_group.0.to_le_bytes());
            hasher.update(slot.storage_view.0.to_le_bytes());
            register_model::sha256_register_units(&mut hasher, &slot.preserved_units);
            register_model::sha256_length(&mut hasher, slot.modified_units.len());
            for requirement in &slot.modified_units {
                hasher.update(requirement.unit.0.to_le_bytes());
                register_model::sha256_length(&mut hasher, requirement.witnesses.len());
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
