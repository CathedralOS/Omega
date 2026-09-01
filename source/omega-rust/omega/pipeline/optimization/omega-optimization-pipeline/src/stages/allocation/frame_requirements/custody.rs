use super::{
    NonAuthoritativeSpillFrameRequirementPlan, NonAuthoritativeSpillFrameRequirementReceipt,
    non_authoritative_spill_frame_requirement_identity,
};

pub(super) fn seal(
    plan: &NonAuthoritativeSpillFrameRequirementPlan,
) -> NonAuthoritativeSpillFrameRequirementReceipt {
    NonAuthoritativeSpillFrameRequirementReceipt {
        identity: non_authoritative_spill_frame_requirement_identity(plan),
        abstract_spill_access_constraints: plan.abstract_spill_access_constraints,
        register_environment: plan.register_environment,
        target: plan.target,
        policy: plan.policy,
        usage: plan.usage,
        function_count: plan.functions.len(),
        spill_bearing_function_count: plan
            .functions
            .iter()
            .filter(|function| function.abstract_spill_area_bytes != 0)
            .count(),
        max_abstract_spill_area_bytes: plan
            .functions
            .iter()
            .map(|function| function.abstract_spill_area_bytes)
            .max()
            .unwrap_or(0),
        max_abstract_spill_area_alignment: plan
            .functions
            .iter()
            .map(|function| function.abstract_spill_area_alignment)
            .max()
            .unwrap_or(1),
    }
}
