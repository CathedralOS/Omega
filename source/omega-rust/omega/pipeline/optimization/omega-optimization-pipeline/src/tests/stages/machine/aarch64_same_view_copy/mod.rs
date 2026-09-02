//! Optimizer module role: stage group. Same-view-copy admission and publication custody.

mod before_compare_i64_left_operand;
mod before_compare_zero;
mod before_return;
mod custody_corruption;
mod publication;

use crate::tests::*;

struct Fixture {
    homes: StagedOptimizedRegisterHomes,
    machine: StagedOptimizedPostAllocationMachinePlan,
}

fn fixture(selection: Optimization, target: NativeTarget) -> Fixture {
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(OptimizationSelections::new([selection]).unwrap()),
    )
    .unwrap();
    let target = lower_optimized_to_target_operations(optimized, target).unwrap();
    let selected = stage_optimized_instruction_selection(target).unwrap();
    let liveness = stage_optimized_liveness(selected).unwrap();
    let ranges = stage_optimized_live_ranges(liveness).unwrap();
    let legality = stage_optimized_allocation_legality(ranges).unwrap();
    let homes = stage_optimized_register_homes(legality).unwrap();
    let machine = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
    Fixture { homes, machine }
}

fn staged_realization(
    selection: Optimization,
) -> StagedPostAllocationMachineFunctionRelativeRealization {
    let fixture = fixture(selection, NativeTarget::linux_arm64());
    let optimization =
        stage_optimized_post_allocation_machine_optimization(&fixture.homes, &fixture.machine)
            .unwrap();
    stage_post_allocation_machine_function_relative_realization(
        fixture.homes,
        fixture.machine,
        optimization,
    )
    .unwrap()
}
