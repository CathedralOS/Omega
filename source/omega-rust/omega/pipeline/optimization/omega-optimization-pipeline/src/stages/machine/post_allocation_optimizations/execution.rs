use omega_optimization_core::Optimization;

use crate::{
    StagedOptimizedPostAllocationMachinePlan, StagedOptimizedRegisterHomes,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
};

use super::{
    OptimizedPostAllocationMachineOptimizationError,
    StagedOptimizedPostAllocationMachineOptimization, selected_rule,
    stage_optimized_aarch64_cbnz_fusion,
    stage_optimized_aarch64_cbnz_fusion_after_selected_lowering,
    stage_optimized_aarch64_movn_materialization,
    stage_optimized_aarch64_movn_materialization_after_selected_lowering,
    stage_optimized_x86_xor_zero_materialization,
    stage_optimized_x86_xor_zero_materialization_after_selected_lowering,
    validate_optimized_aarch64_cbnz_fusion_after_selected_lowering_custody,
    validate_optimized_aarch64_cbnz_fusion_custody,
    validate_optimized_aarch64_movn_materialization_after_selected_lowering_custody,
    validate_optimized_aarch64_movn_materialization_custody,
    validate_optimized_x86_xor_zero_materialization_after_selected_lowering_custody,
    validate_optimized_x86_xor_zero_materialization_custody,
};

pub fn stage_optimized_post_allocation_machine_optimization(
    source: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedPostAllocationMachineOptimization,
    OptimizedPostAllocationMachineOptimizationError,
> {
    let selections = source
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .selections();
    match selected_rule(selections)?.0 {
        Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 => {
            stage_optimized_aarch64_cbnz_fusion(source, machine)
                .map(StagedOptimizedPostAllocationMachineOptimization::Aarch64Cbnz)
        }
        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1 => {
            stage_optimized_aarch64_movn_materialization(source, machine)
                .map(StagedOptimizedPostAllocationMachineOptimization::Aarch64Movn)
        }
        Optimization::X86SelectXorZeroI64MaterializationV1 => {
            stage_optimized_x86_xor_zero_materialization(source, machine)
                .map(StagedOptimizedPostAllocationMachineOptimization::X86XorZero)
        }
        _ => unreachable!("the post-allocation catalog is closed"),
    }
}

pub fn validate_optimized_post_allocation_machine_optimization_custody(
    source: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    staged: &StagedOptimizedPostAllocationMachineOptimization,
) -> Result<(), OptimizedPostAllocationMachineOptimizationError> {
    match staged {
        StagedOptimizedPostAllocationMachineOptimization::Aarch64Cbnz(staged) => {
            validate_optimized_aarch64_cbnz_fusion_custody(source, machine, staged).map(drop)
        }
        StagedOptimizedPostAllocationMachineOptimization::Aarch64Movn(staged) => {
            validate_optimized_aarch64_movn_materialization_custody(source, machine, staged)
                .map(drop)
        }
        StagedOptimizedPostAllocationMachineOptimization::X86XorZero(staged) => {
            validate_optimized_x86_xor_zero_materialization_custody(source, machine, staged)
                .map(drop)
        }
    }
}

pub fn stage_optimized_post_allocation_machine_optimization_after_selected_lowering(
    source: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedPostAllocationMachineOptimization,
    OptimizedPostAllocationMachineOptimizationError,
> {
    let selections = source
        .selected_lowering_run()
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .selections();
    match selected_rule(selections)?.0 {
        Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 => {
            stage_optimized_aarch64_cbnz_fusion_after_selected_lowering(source, machine)
                .map(StagedOptimizedPostAllocationMachineOptimization::Aarch64Cbnz)
        }
        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1 => {
            stage_optimized_aarch64_movn_materialization_after_selected_lowering(source, machine)
                .map(StagedOptimizedPostAllocationMachineOptimization::Aarch64Movn)
        }
        Optimization::X86SelectXorZeroI64MaterializationV1 => {
            stage_optimized_x86_xor_zero_materialization_after_selected_lowering(source, machine)
                .map(StagedOptimizedPostAllocationMachineOptimization::X86XorZero)
        }
        _ => unreachable!("the post-allocation catalog is closed"),
    }
}

pub fn validate_optimized_post_allocation_machine_optimization_after_selected_lowering_custody(
    source: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    staged: &StagedOptimizedPostAllocationMachineOptimization,
) -> Result<(), OptimizedPostAllocationMachineOptimizationError> {
    match staged {
        StagedOptimizedPostAllocationMachineOptimization::Aarch64Cbnz(staged) => {
            validate_optimized_aarch64_cbnz_fusion_after_selected_lowering_custody(
                source, machine, staged,
            )
            .map(drop)
        }
        StagedOptimizedPostAllocationMachineOptimization::Aarch64Movn(staged) => {
            validate_optimized_aarch64_movn_materialization_after_selected_lowering_custody(
                source, machine, staged,
            )
            .map(drop)
        }
        StagedOptimizedPostAllocationMachineOptimization::X86XorZero(staged) => {
            validate_optimized_x86_xor_zero_materialization_after_selected_lowering_custody(
                source, machine, staged,
            )
            .map(drop)
        }
    }
}
