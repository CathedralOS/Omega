use omega_machine_optimizer::{
    PostAllocationMachineRuleCatalogEntry, PostAllocationMachineRuleKind,
};

use super::super::{
    stage_optimized_aarch64_cbnz_fusion,
    stage_optimized_aarch64_cbnz_fusion_after_selected_lowering,
    stage_optimized_aarch64_movn_materialization,
    stage_optimized_aarch64_movn_materialization_after_selected_lowering,
    stage_optimized_x86_mov_r32_imm32_materialization,
    stage_optimized_x86_mov_r32_imm32_materialization_after_active_resident_rematerialization,
    stage_optimized_x86_mov_r32_imm32_materialization_after_selected_lowering,
    stage_optimized_x86_xor_zero_materialization,
    stage_optimized_x86_xor_zero_materialization_after_selected_lowering,
    OptimizedPostAllocationMachineOptimizationError,
    StagedOptimizedPostAllocationMachineOptimization,
};
use crate::{
    StagedOptimizedActiveResidentRematerialization, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedRegisterHomes, StagedOptimizedRegisterHomesAfterSelectedLowering,
};

pub(crate) fn stage_optimized_post_allocation_machine_optimization_for_catalog_entry(
    source: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    entry: PostAllocationMachineRuleCatalogEntry,
) -> Result<
    StagedOptimizedPostAllocationMachineOptimization,
    OptimizedPostAllocationMachineOptimizationError,
> {
    match entry.payload().kind() {
        PostAllocationMachineRuleKind::Aarch64Cbnz => {
            stage_optimized_aarch64_cbnz_fusion(source, machine)
                .map(StagedOptimizedPostAllocationMachineOptimization::Aarch64Cbnz)
        }
        PostAllocationMachineRuleKind::Aarch64Movn => {
            stage_optimized_aarch64_movn_materialization(source, machine)
                .map(StagedOptimizedPostAllocationMachineOptimization::Aarch64Movn)
        }
        PostAllocationMachineRuleKind::X86XorZero => {
            stage_optimized_x86_xor_zero_materialization(source, machine)
                .map(StagedOptimizedPostAllocationMachineOptimization::X86XorZero)
        }
        PostAllocationMachineRuleKind::X86MovR32Imm32 => {
            stage_optimized_x86_mov_r32_imm32_materialization(source, machine)
                .map(StagedOptimizedPostAllocationMachineOptimization::X86MovR32Imm32)
        }
    }
}

pub(crate) fn stage_optimized_post_allocation_machine_optimization_after_selected_lowering_for_catalog_entry(
    source: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    entry: PostAllocationMachineRuleCatalogEntry,
) -> Result<
    StagedOptimizedPostAllocationMachineOptimization,
    OptimizedPostAllocationMachineOptimizationError,
> {
    match entry.payload().kind() {
        PostAllocationMachineRuleKind::Aarch64Cbnz => {
            stage_optimized_aarch64_cbnz_fusion_after_selected_lowering(source, machine)
                .map(StagedOptimizedPostAllocationMachineOptimization::Aarch64Cbnz)
        }
        PostAllocationMachineRuleKind::Aarch64Movn => {
            stage_optimized_aarch64_movn_materialization_after_selected_lowering(source, machine)
                .map(StagedOptimizedPostAllocationMachineOptimization::Aarch64Movn)
        }
        PostAllocationMachineRuleKind::X86XorZero => {
            stage_optimized_x86_xor_zero_materialization_after_selected_lowering(source, machine)
                .map(StagedOptimizedPostAllocationMachineOptimization::X86XorZero)
        }
        PostAllocationMachineRuleKind::X86MovR32Imm32 => {
            stage_optimized_x86_mov_r32_imm32_materialization_after_selected_lowering(
                source, machine,
            )
            .map(StagedOptimizedPostAllocationMachineOptimization::X86MovR32Imm32)
        }
    }
}

pub(crate) fn stage_optimized_post_allocation_machine_optimization_after_active_resident_rematerialization_for_catalog_entry(
    source: &StagedOptimizedActiveResidentRematerialization,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    entry: PostAllocationMachineRuleCatalogEntry,
) -> Result<
    StagedOptimizedPostAllocationMachineOptimization,
    OptimizedPostAllocationMachineOptimizationError,
> {
    match entry.payload().kind() {
        PostAllocationMachineRuleKind::X86MovR32Imm32 => {
            stage_optimized_x86_mov_r32_imm32_materialization_after_active_resident_rematerialization(
                source, machine,
            )
            .map(StagedOptimizedPostAllocationMachineOptimization::X86MovR32Imm32)
        }
        _ => Err(
            OptimizedPostAllocationMachineOptimizationError::UnsupportedPostAllocationMachineOptimization(
                entry.optimization(),
            ),
        ),
    }
}
