use super::{
    AllocatedCalleeSavedRequirementPlan, AllocatedCalleeSavedRequirementReceipt,
    allocated_callee_saved_requirement_identity,
};

pub(super) fn seal(
    plan: &AllocatedCalleeSavedRequirementPlan,
) -> AllocatedCalleeSavedRequirementReceipt {
    AllocatedCalleeSavedRequirementReceipt {
        identity: allocated_callee_saved_requirement_identity(plan),
        selected: plan.selected,
        homes: plan.homes,
        post_allocation_manifest: plan.post_allocation_manifest,
        register_environment: plan.register_environment,
        physical_register_model: plan.physical_register_model,
        target: plan.target,
        abi: plan.abi,
        policy: plan.policy,
        usage: plan.usage,
        function_count: plan.functions.len(),
        modified_function_count: plan
            .functions
            .iter()
            .filter(|function| !function.modified_units.is_empty())
            .count(),
        modified_unit_count: plan
            .functions
            .iter()
            .map(|function| function.modified_units.len())
            .sum(),
        witness_count: plan
            .functions
            .iter()
            .flat_map(|function| &function.modified_units)
            .map(|requirement| requirement.witnesses.len())
            .sum(),
    }
}
