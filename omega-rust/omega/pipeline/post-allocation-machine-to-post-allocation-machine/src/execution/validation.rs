use super::super::{
    OptimizedPostAllocationMachineOptimizationError,
    StagedOptimizedPostAllocationMachineOptimization,
    validate_optimized_aarch64_cbnz_fusion_custody,
    validate_optimized_aarch64_movn_materialization_custody,
    validate_optimized_aarch64_same_view_copy_elision_custody,
    validate_optimized_x86_mov_r32_imm32_materialization_custody,
    validate_optimized_x86_mov_r64_imm32_sign_extended_materialization_custody,
    validate_optimized_x86_xor_zero_materialization_custody,
};
use crate::StagedOptimizedPostAllocationMachinePlan;

pub fn validate_optimized_post_allocation_machine_optimization_custody(
    source: &impl crate::AllocationSource,
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
        StagedOptimizedPostAllocationMachineOptimization::Aarch64SameViewCopyElision(staged) => {
            validate_optimized_aarch64_same_view_copy_elision_custody(source, machine, staged)
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
        StagedOptimizedPostAllocationMachineOptimization::X86MovR64Imm32SignExtended(staged) => {
            validate_optimized_x86_mov_r64_imm32_sign_extended_materialization_custody(
                source, machine, staged,
            )
            .map(drop)
        }
    }
}
