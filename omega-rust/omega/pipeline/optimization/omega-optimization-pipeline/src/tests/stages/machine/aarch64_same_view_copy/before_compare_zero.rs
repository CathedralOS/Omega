use crate::tests::*;
use omega_machine_optimizer::Aarch64SameViewCopyElisionPolicy;
use omega_target::Architecture;

const RULE: Optimization = Optimization::Aarch64ElideSameViewCopyI64BeforeCompareZeroV1;

#[test]
fn exact_selection_is_deterministic_and_retains_distinct_policy_custody() {
    let fixture = super::fixture(RULE, NativeTarget::linux_arm64());
    let first =
        stage_optimized_post_allocation_machine_optimization(&fixture.homes, &fixture.machine)
            .unwrap();
    let second =
        stage_optimized_post_allocation_machine_optimization(&fixture.homes, &fixture.machine)
            .unwrap();

    assert_eq!(first, second);
    assert_eq!(first.optimization(), RULE);
    assert_eq!(first.action_count(), 0);
    let StagedOptimizedPostAllocationMachineOptimization::Aarch64SameViewCopyElision(elision) =
        &first
    else {
        panic!("the exact before-compare selection must retain same-view-copy custody")
    };
    assert_eq!(
        elision.elision().plan().policy,
        Aarch64SameViewCopyElisionPolicy::Aarch64ElideSameViewCopyI64BeforeCompareZeroV1
    );
    assert_eq!(elision.elision().plan().budget, budget());

    let realization = stage_post_allocation_machine_function_relative_realization(
        fixture.homes,
        fixture.machine,
        first,
    )
    .unwrap();
    assert_eq!(realization.optimization().optimization(), RULE);
    assert_eq!(realization.custody().optimization().action_count(), 0);
    validate_post_allocation_machine_function_relative_realization_custody(&realization).unwrap();
}

#[test]
fn hosted_aarch64_targets_reach_object_and_callable_publication() {
    for target in [NativeTarget::linux_arm64(), NativeTarget::macos_arm64()] {
        super::publication::assert_no_candidate_reaches_object_and_callable(RULE, target);
    }
}

#[test]
fn absent_exact_selection_and_wrong_architecture_fail_before_rule_execution() {
    let disabled = super::fixture(Optimization::CopyPropagation, NativeTarget::linux_arm64());
    assert_eq!(
        stage_optimized_aarch64_same_view_copy_before_compare_zero_elision(
            &disabled.homes,
            &disabled.machine,
        ),
        Err(
            OptimizedPostAllocationMachineOptimizationError::MissingPostAllocationMachineOptimization
        )
    );

    let wrong_target = super::fixture(RULE, NativeTarget::linux_x64());
    assert_eq!(
        stage_optimized_post_allocation_machine_optimization(
            &wrong_target.homes,
            &wrong_target.machine,
        ),
        Err(
            OptimizedPostAllocationMachineOptimizationError::UnsupportedPostAllocationMachineOptimizationTarget {
                optimization: RULE,
                required: Architecture::Aarch64,
                actual: Architecture::X86_64,
            }
        )
    );
}
