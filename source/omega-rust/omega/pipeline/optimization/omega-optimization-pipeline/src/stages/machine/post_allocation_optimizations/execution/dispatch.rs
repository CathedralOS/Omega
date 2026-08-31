use omega_optimization_core::Optimization;

use super::super::{
    OptimizedPostAllocationMachineOptimizationError,
    StagedOptimizedPostAllocationMachineOptimization, stage_optimized_aarch64_cbnz_fusion,
    stage_optimized_aarch64_cbnz_fusion_after_selected_lowering,
    stage_optimized_aarch64_movn_materialization,
    stage_optimized_aarch64_movn_materialization_after_selected_lowering,
    stage_optimized_x86_mov_r32_imm32_materialization,
    stage_optimized_x86_mov_r32_imm32_materialization_after_selected_lowering,
    stage_optimized_x86_xor_zero_materialization,
    stage_optimized_x86_xor_zero_materialization_after_selected_lowering,
};
use crate::{
    StagedOptimizedPostAllocationMachinePlan, StagedOptimizedRegisterHomes,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
};

pub(crate) fn stage_optimized_post_allocation_machine_optimization_for_rule(
    source: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    rule: Optimization,
) -> Result<
    StagedOptimizedPostAllocationMachineOptimization,
    OptimizedPostAllocationMachineOptimizationError,
> {
    match rule {
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
        Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1 => {
            stage_optimized_x86_mov_r32_imm32_materialization(source, machine)
                .map(StagedOptimizedPostAllocationMachineOptimization::X86MovR32Imm32)
        }
        _ => unreachable!("the post-allocation catalog is closed"),
    }
}

pub(crate) fn stage_optimized_post_allocation_machine_optimization_after_selected_lowering_for_rule(
    source: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    rule: Optimization,
) -> Result<
    StagedOptimizedPostAllocationMachineOptimization,
    OptimizedPostAllocationMachineOptimizationError,
> {
    match rule {
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
        Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1 => {
            stage_optimized_x86_mov_r32_imm32_materialization_after_selected_lowering(
                source, machine,
            )
            .map(StagedOptimizedPostAllocationMachineOptimization::X86MovR32Imm32)
        }
        _ => unreachable!("the post-allocation catalog is closed"),
    }
}
