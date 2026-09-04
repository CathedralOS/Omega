use omega_machine_optimizer::{
    PostAllocationMachineRuleCatalogEntry, PostAllocationMachineRuleKind,
};

use super::super::{
    OptimizedPostAllocationMachineOptimizationError,
    StagedOptimizedPostAllocationMachineOptimization, stage_optimized_aarch64_cbnz_fusion,
    stage_optimized_aarch64_cbnz_fusion_after_selected_lowering,
    stage_optimized_aarch64_movn_materialization,
    stage_optimized_aarch64_movn_materialization_after_active_resident_rematerialization,
    stage_optimized_aarch64_movn_materialization_after_selected_lowering,
    stage_optimized_aarch64_same_view_copy_before_compare_i64_left_operand_elision,
    stage_optimized_aarch64_same_view_copy_before_compare_i64_left_operand_elision_after_selected_lowering,
    stage_optimized_aarch64_same_view_copy_before_compare_i64_right_operand_elision,
    stage_optimized_aarch64_same_view_copy_before_compare_i64_right_operand_elision_after_selected_lowering,
    stage_optimized_aarch64_same_view_copy_before_compare_zero_elision,
    stage_optimized_aarch64_same_view_copy_before_compare_zero_elision_after_selected_lowering,
    stage_optimized_aarch64_same_view_copy_elision,
    stage_optimized_aarch64_same_view_copy_elision_after_selected_lowering,
    stage_optimized_x86_mov_r32_imm32_materialization,
    stage_optimized_x86_mov_r32_imm32_materialization_after_active_resident_rematerialization,
    stage_optimized_x86_mov_r32_imm32_materialization_after_selected_lowering,
    stage_optimized_x86_mov_r64_imm32_sign_extended_materialization,
    stage_optimized_x86_mov_r64_imm32_sign_extended_materialization_after_active_resident_rematerialization,
    stage_optimized_x86_mov_r64_imm32_sign_extended_materialization_after_selected_lowering,
    stage_optimized_x86_xor_zero_materialization,
    stage_optimized_x86_xor_zero_materialization_after_active_resident_rematerialization,
    stage_optimized_x86_xor_zero_materialization_after_selected_lowering,
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
        PostAllocationMachineRuleKind::Aarch64SameViewCopyElision => {
            stage_optimized_aarch64_same_view_copy_elision(source, machine)
                .map(StagedOptimizedPostAllocationMachineOptimization::Aarch64SameViewCopyElision)
        }
        PostAllocationMachineRuleKind::Aarch64SameViewCopyBeforeCompareZeroElision => {
            stage_optimized_aarch64_same_view_copy_before_compare_zero_elision(source, machine)
                .map(StagedOptimizedPostAllocationMachineOptimization::Aarch64SameViewCopyElision)
        }
        PostAllocationMachineRuleKind::Aarch64SameViewCopyBeforeCompareI64LeftOperandElision => {
            stage_optimized_aarch64_same_view_copy_before_compare_i64_left_operand_elision(
                source, machine,
            )
            .map(StagedOptimizedPostAllocationMachineOptimization::Aarch64SameViewCopyElision)
        }
        PostAllocationMachineRuleKind::Aarch64SameViewCopyBeforeCompareI64RightOperandElision => {
            stage_optimized_aarch64_same_view_copy_before_compare_i64_right_operand_elision(
                source, machine,
            )
            .map(StagedOptimizedPostAllocationMachineOptimization::Aarch64SameViewCopyElision)
        }
        PostAllocationMachineRuleKind::X86XorZero => {
            stage_optimized_x86_xor_zero_materialization(source, machine)
                .map(StagedOptimizedPostAllocationMachineOptimization::X86XorZero)
        }
        PostAllocationMachineRuleKind::X86MovR32Imm32 => {
            stage_optimized_x86_mov_r32_imm32_materialization(source, machine)
                .map(StagedOptimizedPostAllocationMachineOptimization::X86MovR32Imm32)
        }
        PostAllocationMachineRuleKind::X86MovR64Imm32SignExtended => {
            stage_optimized_x86_mov_r64_imm32_sign_extended_materialization(source, machine)
                .map(StagedOptimizedPostAllocationMachineOptimization::X86MovR64Imm32SignExtended)
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
        PostAllocationMachineRuleKind::Aarch64SameViewCopyElision => {
            stage_optimized_aarch64_same_view_copy_elision_after_selected_lowering(source, machine)
                .map(StagedOptimizedPostAllocationMachineOptimization::Aarch64SameViewCopyElision)
        }
        PostAllocationMachineRuleKind::Aarch64SameViewCopyBeforeCompareZeroElision => {
            stage_optimized_aarch64_same_view_copy_before_compare_zero_elision_after_selected_lowering(source, machine)
                .map(StagedOptimizedPostAllocationMachineOptimization::Aarch64SameViewCopyElision)
        }
        PostAllocationMachineRuleKind::Aarch64SameViewCopyBeforeCompareI64LeftOperandElision => {
            stage_optimized_aarch64_same_view_copy_before_compare_i64_left_operand_elision_after_selected_lowering(
                source, machine,
            )
            .map(StagedOptimizedPostAllocationMachineOptimization::Aarch64SameViewCopyElision)
        }
        PostAllocationMachineRuleKind::Aarch64SameViewCopyBeforeCompareI64RightOperandElision => {
            stage_optimized_aarch64_same_view_copy_before_compare_i64_right_operand_elision_after_selected_lowering(
                source, machine,
            )
            .map(StagedOptimizedPostAllocationMachineOptimization::Aarch64SameViewCopyElision)
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
        PostAllocationMachineRuleKind::X86MovR64Imm32SignExtended => {
            stage_optimized_x86_mov_r64_imm32_sign_extended_materialization_after_selected_lowering(
                source, machine,
            )
            .map(StagedOptimizedPostAllocationMachineOptimization::X86MovR64Imm32SignExtended)
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
        PostAllocationMachineRuleKind::X86XorZero => {
            stage_optimized_x86_xor_zero_materialization_after_active_resident_rematerialization(
                source, machine,
            )
            .map(StagedOptimizedPostAllocationMachineOptimization::X86XorZero)
        }
        PostAllocationMachineRuleKind::Aarch64Movn => {
            stage_optimized_aarch64_movn_materialization_after_active_resident_rematerialization(
                source, machine,
            )
            .map(StagedOptimizedPostAllocationMachineOptimization::Aarch64Movn)
        }
        PostAllocationMachineRuleKind::X86MovR32Imm32 => {
            stage_optimized_x86_mov_r32_imm32_materialization_after_active_resident_rematerialization(
                source, machine,
            )
            .map(StagedOptimizedPostAllocationMachineOptimization::X86MovR32Imm32)
        }
        PostAllocationMachineRuleKind::X86MovR64Imm32SignExtended => {
            stage_optimized_x86_mov_r64_imm32_sign_extended_materialization_after_active_resident_rematerialization(
                source, machine,
            )
            .map(
                StagedOptimizedPostAllocationMachineOptimization::X86MovR64Imm32SignExtended,
            )
        }
        _ => Err(
            OptimizedPostAllocationMachineOptimizationError::UnsupportedPostAllocationMachineOptimization(
                entry.optimization(),
            ),
        ),
    }
}
