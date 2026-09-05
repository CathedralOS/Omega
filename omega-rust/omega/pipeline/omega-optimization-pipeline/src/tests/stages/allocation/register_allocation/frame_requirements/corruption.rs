use crate::tests::*;

use super::fixture::{exact_budget, spill_source, stage};

#[test]
fn replay_rejects_roots_usage_and_every_retained_requirement_field() {
    let target = NativeTarget::linux_x64();
    let source = spill_source(target);
    let environment = baseline_target_register_environment(target).unwrap();
    let canonical = stage(&source, &environment, exact_budget())
        .unwrap()
        .plan()
        .clone();

    let mut root = canonical.clone();
    root.abstract_spill_access_constraints =
        omega_selected_instructions_to_register_homes::AbstractSpillAccessConstraintPlanIdentity::from_bytes([0x51; 32]);
    assert_eq!(
        validate_non_authoritative_spill_frame_requirements(&source, &environment, root),
        Err(SpillFrameRequirementError::RootMismatch),
    );

    let mut target_root = canonical.clone();
    target_root.target = NativeTarget::linux_arm64();
    assert_eq!(
        validate_non_authoritative_spill_frame_requirements(&source, &environment, target_root,),
        Err(SpillFrameRequirementError::RootMismatch),
    );

    let mut environment_root = canonical.clone();
    environment_root.register_environment =
        omega_register_model::TargetRegisterEnvironmentIdentity::from_bytes([0x52; 32]);
    assert_eq!(
        validate_non_authoritative_spill_frame_requirements(
            &source,
            &environment,
            environment_root,
        ),
        Err(SpillFrameRequirementError::RootMismatch),
    );

    let mut usage = canonical.clone();
    usage.usage.validation_steps += 1;
    assert_eq!(
        validate_non_authoritative_spill_frame_requirements(&source, &environment, usage),
        Err(SpillFrameRequirementError::UsageMismatch),
    );

    for corrupt in [
        |plan: &mut NonAuthoritativeSpillFrameRequirementPlan| {
            plan.functions[0].abstract_spill_area_bytes += 8;
        },
        |plan: &mut NonAuthoritativeSpillFrameRequirementPlan| {
            plan.functions[0].abstract_spill_area_alignment *= 2;
        },
        |plan: &mut NonAuthoritativeSpillFrameRequirementPlan| {
            plan.functions[0].abi_preservation_convention =
                FrameAbiPreservationConvention::MicrosoftX64;
        },
        |plan: &mut NonAuthoritativeSpillFrameRequirementPlan| {
            plan.functions[0].abi_stack_alignment *= 2;
        },
        |plan: &mut NonAuthoritativeSpillFrameRequirementPlan| {
            plan.functions[0].abi_red_zone_capacity_bytes = 0;
        },
        |plan: &mut NonAuthoritativeSpillFrameRequirementPlan| {
            plan.functions[0].machine = psi_core::MachineId::new(52_991).unwrap();
        },
        |plan: &mut NonAuthoritativeSpillFrameRequirementPlan| {
            plan.functions.clear();
        },
    ] {
        let mut changed = canonical.clone();
        corrupt(&mut changed);
        assert_eq!(
            validate_non_authoritative_spill_frame_requirements(&source, &environment, changed,),
            Err(SpillFrameRequirementError::NonCanonicalRequirements),
        );
    }

    let mut under_budget = canonical;
    under_budget.budget =
        omega_optimization_core::OptimizationWorkBudget::new(1, 6, 7, 2, 7).unwrap();
    assert_eq!(
        validate_non_authoritative_spill_frame_requirements(&source, &environment, under_budget,),
        Err(SpillFrameRequirementError::BudgetExceeded {
            required: super::fixture::EXACT_USAGE,
            budget: omega_optimization_core::OptimizationWorkBudget::new(1, 6, 7, 2, 7).unwrap(),
        }),
    );
}

#[test]
fn mismatched_authenticated_environment_fails_before_planning() {
    let source = spill_source(NativeTarget::linux_x64());
    let foreign = baseline_target_register_environment(NativeTarget::windows_x64()).unwrap();
    assert_eq!(
        stage(&source, &foreign, exact_budget()),
        Err(SpillFrameRequirementError::RootMismatch),
    );
}
