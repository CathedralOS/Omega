use sha2::{Digest, Sha256};

use super::{
    AllocatedCalleeSavedRequirementIdentity, AllocatedCalleeSavedRequirementPlan,
    AllocatedCalleeSavedRequirementPolicy, encode_callee_saved_modification_witness_identity,
};

pub(crate) fn allocated_callee_saved_requirement_identity(
    plan: &AllocatedCalleeSavedRequirementPlan,
) -> AllocatedCalleeSavedRequirementIdentity {
    let mut hasher = Sha256::new();
    hasher.update(b"omega.allocated-callee-saved-requirements.v2");
    hasher.update(plan.selected.bytes());
    hasher.update(plan.homes.bytes());
    hasher.update(plan.post_allocation_manifest.bytes());
    hasher.update(plan.register_environment.bytes());
    hasher.update(plan.physical_register_model.bytes());
    plan.target.encode_identity(&mut hasher);
    hasher.update([plan.abi.abi_tag()]);
    register_model::sha256_register_units(&mut hasher, &plan.callee_saved_units);
    hasher.update([match plan.policy {
        AllocatedCalleeSavedRequirementPolicy::AllocatedSelectedWritesIntersectAbiPreservationV1 => 0,
    }]);
    hasher.update(plan.budget.encode());
    hasher.update(plan.usage.encode());
    register_model::sha256_length(&mut hasher, plan.functions.len());
    for function in &plan.functions {
        hasher.update(function.machine.get().to_le_bytes());
        register_model::sha256_length(&mut hasher, function.modified_units.len());
        for requirement in &function.modified_units {
            hasher.update(requirement.unit.0.to_le_bytes());
            register_model::sha256_length(&mut hasher, requirement.witnesses.len());
            for witness in &requirement.witnesses {
                encode_callee_saved_modification_witness_identity(&mut hasher, *witness);
            }
        }
    }
    AllocatedCalleeSavedRequirementIdentity::from_bytes(hasher.finalize().into())
}
