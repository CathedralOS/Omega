//! Rule-owned CBNZ fixtures with no production admission authority.

use crate::tests::*;

pub(super) struct OperationalFixture {
    pub(super) homes: StagedOptimizedRegisterHomes,
    pub(super) machine: StagedOptimizedPostAllocationMachinePlan,
}

/// Two independent functions give the rule two selectable terminal pairs.
pub(super) fn operational_fixture() -> OperationalFixture {
    let (semantic, proof) = disconnected_conditional_artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(
            OptimizationSelections::new([
                Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
            ])
            .unwrap(),
        ),
    )
    .unwrap();
    let target =
        lower_optimized_to_target_operations(optimized, NativeTarget::linux_arm64()).unwrap();
    let selected = stage_optimized_instruction_selection(target).unwrap();
    let liveness = stage_optimized_liveness(selected).unwrap();
    let ranges = stage_optimized_live_ranges(liveness).unwrap();
    let legality = stage_optimized_allocation_legality(ranges).unwrap();
    let homes = stage_optimized_register_homes(legality).unwrap();
    let machine = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
    OperationalFixture { homes, machine }
}

/// Canonical direct-source construction for generic custody mutation tests.
pub(super) fn staged_realization() -> StagedPostAllocationMachineFunctionRelativeRealization {
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let selections =
        OptimizationSelections::new([Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1])
            .unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let target =
        lower_optimized_to_target_operations(optimized, NativeTarget::linux_arm64()).unwrap();
    let selected = stage_optimized_instruction_selection(target).unwrap();
    let liveness = stage_optimized_liveness(selected).unwrap();
    let ranges = stage_optimized_live_ranges(liveness).unwrap();
    let legality = stage_optimized_allocation_legality(ranges).unwrap();
    let homes = stage_optimized_register_homes(legality).unwrap();
    let machine = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
    let optimization =
        stage_optimized_post_allocation_machine_optimization(&homes, &machine).unwrap();
    stage_post_allocation_machine_function_relative_realization(homes, machine, optimization)
        .unwrap()
}
