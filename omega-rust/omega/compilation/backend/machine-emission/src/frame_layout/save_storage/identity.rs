
use crate::frame_layout::save_storage::encode_callee_saved_modification_witness_identity;

use super::{
    NonAuthoritativeCalleeSaveStorageIdentity, NonAuthoritativeCalleeSaveStoragePlan,
    NonAuthoritativeCalleeSaveStoragePolicy,
};

pub(crate) fn non_authoritative_callee_save_storage_identity(
    plan: &NonAuthoritativeCalleeSaveStoragePlan,
) -> NonAuthoritativeCalleeSaveStorageIdentity {
    let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(b"omega.non-authoritative-callee-save-storage.v2");
    hasher.raw(&plan.callee_saved_requirements.bytes());
    hasher.raw(&plan.register_environment.bytes());
    hasher.raw(&plan.physical_register_model.bytes());
    hasher.raw(&plan.preservation_storage_catalog.bytes());
    hasher.raw(&plan.target.identity_bytes());
    hasher.byte(plan.abi.abi_tag());
    register_model::sha256_register_units(&mut hasher, &plan.callee_saved_units);
    hasher.byte(match plan.policy {
        NonAuthoritativeCalleeSaveStoragePolicy::CanonicalTargetPreservationGroupsV1 => 0,
    });
    hasher.raw(&plan.budget.encode());
    hasher.raw(&plan.usage.encode());
    register_model::sha256_length(&mut hasher, plan.functions.len());
    for function in &plan.functions {
        hasher.raw(&function.machine.get().to_le_bytes());
        hasher.raw(&function.abstract_area_bytes.to_le_bytes());
        hasher.raw(&function.abstract_area_alignment.to_le_bytes());
        register_model::sha256_length(&mut hasher, function.slots.len());
        for slot in &function.slots {
            hasher.raw(&slot.id.0.to_le_bytes());
            hasher.raw(&slot.storage_group.0.to_le_bytes());
            hasher.raw(&slot.storage_view.0.to_le_bytes());
            register_model::sha256_register_units(&mut hasher, &slot.preserved_units);
            register_model::sha256_length(&mut hasher, slot.modified_units.len());
            for requirement in &slot.modified_units {
                hasher.raw(&requirement.unit.0.to_le_bytes());
                register_model::sha256_length(&mut hasher, requirement.witnesses.len());
                for witness in &requirement.witnesses {
                    encode_callee_saved_modification_witness_identity(&mut hasher, *witness);
                }
            }
            hasher.raw(&slot.abstract_offset_bytes.to_le_bytes());
            hasher.raw(&slot.size_bytes.to_le_bytes());
            hasher.raw(&slot.alignment_bytes.to_le_bytes());
        }
    }
    NonAuthoritativeCalleeSaveStorageIdentity::from_bytes(hasher.finish().into())
}
