//! Independent indexed replay of spill geometry and ABI selection.

use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_regalloc::ValidatedAbstractSpillAccessConstraints;

use crate::ValidatedTargetRegisterEnvironment;

use super::{
    FrameAbiPreservationConvention, FunctionSpillFrameRequirements,
    NonAuthoritativeSpillFrameRequirementPolicy, SpillFrameRequirementError,
};

pub(super) fn reconstruct(
    source: &ValidatedAbstractSpillAccessConstraints,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: NonAuthoritativeSpillFrameRequirementPolicy,
    budget: OptimizationWorkBudget,
) -> Result<(Vec<FunctionSpillFrameRequirements>, OptimizationWorkUsage), SpillFrameRequirementError>
{
    if policy
        != NonAuthoritativeSpillFrameRequirementPolicy::AbstractSpillAreaAndPreservationConventionV1
    {
        return Err(SpillFrameRequirementError::UnsupportedPolicy);
    }
    if source.receipt().register_environment() != environment.identity() {
        return Err(SpillFrameRequirementError::RootMismatch);
    }
    let target = environment.target();
    let (abi, convention) = match (target.architecture, target.object_format) {
        (omega_target::Architecture::X86_64, omega_target::ObjectFormat::Elf) => (
            FrameAbiPreservationConvention::SystemVAMD64,
            omega_isa_x86_64::x86_64_preservation_convention_for_target(
                environment.physical(),
                target,
            ),
        ),
        (omega_target::Architecture::X86_64, omega_target::ObjectFormat::Coff) => (
            FrameAbiPreservationConvention::MicrosoftX64,
            omega_isa_x86_64::x86_64_preservation_convention_for_target(
                environment.physical(),
                target,
            ),
        ),
        (omega_target::Architecture::Aarch64, omega_target::ObjectFormat::Elf) => (
            FrameAbiPreservationConvention::Aapcs64,
            omega_isa_aarch64::aarch64_preservation_convention_for_target(
                environment.physical(),
                target,
            ),
        ),
        (omega_target::Architecture::Aarch64, omega_target::ObjectFormat::MachO) => (
            FrameAbiPreservationConvention::DarwinAapcs64,
            omega_isa_aarch64::aarch64_preservation_convention_for_target(
                environment.physical(),
                target,
            ),
        ),
        _ => return Err(SpillFrameRequirementError::UnsupportedTargetConvention),
    };
    let convention = convention.ok_or(SpillFrameRequirementError::UnsupportedTargetConvention)?;
    let mut functions = Vec::with_capacity(source.plan().functions.len());
    let mut placement_count = 0_u64;
    for function in &source.plan().functions {
        let mut alignment = 1_u64;
        for placement in &function.placements {
            alignment = alignment.max(placement.alignment_bytes);
            placement_count = placement_count
                .checked_add(1)
                .ok_or(SpillFrameRequirementError::WorkOverflow)?;
        }
        functions.push(reconstruct_function(
            function,
            alignment,
            abi,
            convention.stack_alignment,
            convention.red_zone_bytes,
        ));
    }
    let function_count =
        u64::try_from(functions.len()).map_err(|_| SpillFrameRequirementError::WorkOverflow)?;
    let usage = OptimizationWorkUsage {
        rule_evaluations: function_count
            .checked_add(1)
            .ok_or(SpillFrameRequirementError::WorkOverflow)?,
        candidates: placement_count,
        validation_steps: function_count
            .checked_add(placement_count)
            .ok_or(SpillFrameRequirementError::WorkOverflow)?,
        commits: function_count
            .checked_add(1)
            .ok_or(SpillFrameRequirementError::WorkOverflow)?,
        iterations: function_count
            .checked_add(placement_count)
            .ok_or(SpillFrameRequirementError::WorkOverflow)?,
    };
    if !usage.within(budget) {
        return Err(SpillFrameRequirementError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    Ok((functions, usage))
}

fn reconstruct_function(
    function: &omega_regalloc::FunctionAbstractSpillAccessConstraints,
    alignment: u64,
    abi: FrameAbiPreservationConvention,
    stack_alignment: u16,
    red_zone_capacity_bytes: u16,
) -> FunctionSpillFrameRequirements {
    FunctionSpillFrameRequirements {
        machine: function.machine,
        abstract_spill_area_bytes: function.spill_area_bytes,
        abstract_spill_area_alignment: alignment,
        abi_preservation_convention: abi,
        abi_stack_alignment: stack_alignment,
        abi_red_zone_capacity_bytes: red_zone_capacity_bytes,
    }
}

#[cfg(test)]
pub(crate) fn replay_zero_access_requirement_for_test(
    machine: psi_core::MachineId,
) -> FunctionSpillFrameRequirements {
    reconstruct_function(
        &omega_regalloc::FunctionAbstractSpillAccessConstraints {
            machine,
            spill_area_bytes: 0,
            placements: Vec::new(),
            dependencies: Vec::new(),
        },
        1,
        FrameAbiPreservationConvention::SystemVAMD64,
        16,
        128,
    )
}
