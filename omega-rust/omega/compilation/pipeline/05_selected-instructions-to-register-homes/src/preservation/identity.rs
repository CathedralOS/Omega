
use super::{
    AllocatedCalleeSavedRequirementIdentity, AllocatedCalleeSavedRequirementPlan,
    AllocatedCalleeSavedRequirementPolicy, encode_callee_saved_modification_witness_identity,
};

pub(crate) fn allocated_callee_saved_requirement_identity(
    plan: &AllocatedCalleeSavedRequirementPlan,
) -> AllocatedCalleeSavedRequirementIdentity {
    let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(b"omega.allocated-callee-saved-requirements.v2");
    hasher.raw(&plan.selected.bytes());
    hasher.raw(&plan.homes.bytes());
    hasher.raw(&plan.post_allocation_manifest.bytes());
    hasher.raw(&plan.register_environment.bytes());
    hasher.raw(&plan.physical_register_model.bytes());
    hasher.raw(&plan.target.identity_bytes());
    hasher.byte(plan.abi.abi_tag());
    register_model::sha256_register_units(&mut hasher, &plan.callee_saved_units);
    hasher.byte(match plan.policy {
        AllocatedCalleeSavedRequirementPolicy::AllocatedSelectedWritesIntersectAbiPreservationV1 => 0,
    });
    hasher.raw(&plan.budget.encode());
    hasher.raw(&plan.usage.encode());
    register_model::sha256_length(&mut hasher, plan.functions.len());
    for function in &plan.functions {
        hasher.raw(&function.machine.get().to_le_bytes());
        register_model::sha256_length(&mut hasher, function.modified_units.len());
        for requirement in &function.modified_units {
            hasher.raw(&requirement.unit.0.to_le_bytes());
            register_model::sha256_length(&mut hasher, requirement.witnesses.len());
            for witness in &requirement.witnesses {
                encode_callee_saved_modification_witness_identity(&mut hasher, *witness);
            }
        }
    }
    AllocatedCalleeSavedRequirementIdentity::from_bytes(hasher.finish().into())
}
