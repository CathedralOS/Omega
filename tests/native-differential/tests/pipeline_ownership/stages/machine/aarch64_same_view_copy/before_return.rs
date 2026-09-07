use crate::tests::*;
use target::Architecture;

const RULE: Optimization = Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1;

#[test]
fn exact_selection_elides_both_ordinary_return_snapshots_and_preserves_custody() {
    let fixture = super::fixture(RULE, NativeTarget::linux_arm64());
    let first =
        stage_optimized_post_allocation_machine_optimization(&fixture.homes, &fixture.machine)
            .unwrap();
    let second =
        stage_optimized_post_allocation_machine_optimization(&fixture.homes, &fixture.machine)
            .unwrap();
    assert_eq!(first, second);
    assert_eq!(first.optimization(), RULE);
    assert_eq!(first.action_count(), 2);
    let StagedOptimizedPostAllocationMachineOptimization::Aarch64SameViewCopyElision(elision) =
        &first
    else {
        panic!("the exact selection must retain same-view-copy custody")
    };
    assert_eq!(elision.elision().plan().budget, budget());

    let realization = stage_post_allocation_machine_function_relative_realization(
        fixture.homes,
        fixture.machine,
        first,
    )
    .unwrap();
    assert_eq!(realization.optimization().optimization(), RULE);
    assert_eq!(realization.custody().optimization().action_count(), 2);
    assert_eq!(
        realization.custody().optimization().expected_byte_savings(),
        Some(8)
    );
    let baseline_bytes: u64 = realization
        .baseline_layout()
        .functions()
        .iter()
        .map(|function| function.byte_count)
        .sum();
    let current_bytes: u64 = realization
        .layout()
        .functions()
        .iter()
        .map(|function| function.byte_count)
        .sum();
    assert_eq!(baseline_bytes - current_bytes, 8);
    validate_post_allocation_machine_function_relative_realization_custody(&realization).unwrap();
}

#[test]
fn compiler_generated_return_elisions_reach_object_and_callable_publication() {
    super::publication::assert_reaches_object_and_callable(RULE, NativeTarget::linux_arm64(), 2);
}

#[test]
fn absent_exact_selection_and_wrong_architecture_fail_before_rule_execution() {
    let disabled = super::fixture(Optimization::CopyPropagation, NativeTarget::linux_arm64());
    assert_eq!(
        stage_optimized_aarch64_same_view_copy_elision(&disabled.homes, &disabled.machine),
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
