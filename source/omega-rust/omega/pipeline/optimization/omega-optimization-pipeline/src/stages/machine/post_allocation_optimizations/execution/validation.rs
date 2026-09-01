use super::super::{
    validate_optimized_aarch64_cbnz_fusion_after_selected_lowering_custody,
    validate_optimized_aarch64_cbnz_fusion_custody,
    validate_optimized_aarch64_movn_materialization_after_selected_lowering_custody,
    validate_optimized_aarch64_movn_materialization_custody,
    validate_optimized_x86_mov_r32_imm32_materialization_after_active_resident_rematerialization_custody,
    validate_optimized_x86_mov_r32_imm32_materialization_after_selected_lowering_custody,
    validate_optimized_x86_mov_r32_imm32_materialization_custody,
    validate_optimized_x86_xor_zero_materialization_after_selected_lowering_custody,
    validate_optimized_x86_xor_zero_materialization_custody,
    OptimizedPostAllocationMachineOptimizationError,
    StagedOptimizedPostAllocationMachineOptimization,
};
use crate::{
    StagedOptimizedActiveResidentRematerialization, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedRegisterHomes, StagedOptimizedRegisterHomesAfterSelectedLowering,
};

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
        StagedOptimizedPostAllocationMachineOptimization::X86MovR32Imm32(staged) => {
            validate_optimized_x86_mov_r32_imm32_materialization_custody(source, machine, staged)
                .map(drop)
        }
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
        StagedOptimizedPostAllocationMachineOptimization::X86MovR32Imm32(staged) => {
            validate_optimized_x86_mov_r32_imm32_materialization_after_selected_lowering_custody(
                source, machine, staged,
            )
            .map(drop)
        }
    }
}

pub fn validate_optimized_post_allocation_machine_optimization_after_active_resident_rematerialization_custody(
    source: &StagedOptimizedActiveResidentRematerialization,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    staged: &StagedOptimizedPostAllocationMachineOptimization,
) -> Result<(), OptimizedPostAllocationMachineOptimizationError> {
    match staged {
        StagedOptimizedPostAllocationMachineOptimization::X86MovR32Imm32(staged) => {
            validate_optimized_x86_mov_r32_imm32_materialization_after_active_resident_rematerialization_custody(
                source, machine, staged,
            )
            .map(drop)
        }
        _ => Err(
            OptimizedPostAllocationMachineOptimizationError::UnsupportedPostAllocationMachineOptimization(
                staged.optimization(),
            ),
        ),
    }
}
