use omega_effects::{CapabilityFlowPlan, EffectPlan};
use omega_typed_trees::TypedTrees;

pub(crate) fn build_capability_facts(
    _program: &TypedTrees,
    _effects: &EffectPlan,
) -> CapabilityFlowPlan {
    CapabilityFlowPlan::empty()
}
