use crate::tests::*;
use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use super::super::{
    homed_spill_pseudo_instructions,
    recursive_reload_value_homes::{Bundle, reload_bundle},
};

pub(super) const EXACT_USAGE: OptimizationWorkUsage = OptimizationWorkUsage {
    rule_evaluations: 2,
    candidates: 6,
    validation_steps: 7,
    commits: 2,
    iterations: 7,
};

pub(super) fn exact_budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(2, 6, 7, 2, 7).unwrap()
}

pub(super) fn spill_source(
    target: NativeTarget,
) -> omega_selected_instructions_to_register_homes::ValidatedAbstractSpillAccessConstraints {
    let source =
        homed_spill_pseudo_instructions::build(reload_bundle as fn(NativeTarget) -> Bundle, target);
    let homed =
        homed_spill_pseudo_instructions::lower(&source, selected_lowering_budget()).unwrap();
    let effects = omega_selected_instructions_to_register_homes::derive_abstract_spill_memory_effects(
        &homed,
        omega_selected_instructions_to_register_homes::AbstractSpillMemoryEffectPolicy::HomedPseudoReadWriteV1,
        selected_lowering_budget(),
    )
    .unwrap();
    omega_selected_instructions_to_register_homes::constrain_abstract_spill_accesses(
        &effects,
        omega_selected_instructions_to_register_homes::AbstractSpillAccessConstraintPolicy::BlockLocalDataBarrierAndOverlapV1,
        selected_lowering_budget(),
    )
    .unwrap()
}

pub(super) fn stage(
    source: &omega_selected_instructions_to_register_homes::ValidatedAbstractSpillAccessConstraints,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedNonAuthoritativeSpillFrameRequirements, SpillFrameRequirementError> {
    stage_non_authoritative_spill_frame_requirements(
        source,
        environment,
        NonAuthoritativeSpillFrameRequirementPolicy::AbstractSpillAreaAndPreservationConventionV1,
        budget,
    )
}
