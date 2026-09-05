//! Independent indexed replay of spill geometry and ABI selection.

use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_selected_instructions_to_register_homes::ValidatedAbstractSpillAccessConstraints;

use crate::ValidatedTargetRegisterEnvironment;
use omega_target_to_register_environment::selected_abi_preservation;

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
    let selected = selected_abi_preservation(environment)
        .map_err(|_| SpillFrameRequirementError::UnsupportedTargetConvention)?;
    let abi = selected.kind;
    let convention = selected.convention;
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
    function: &omega_selected_instructions_to_register_homes::FunctionAbstractSpillAccessConstraints,
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
pub(in crate::spill_requirements) fn replay_zero_access_requirement_for_test(
    machine: psi_core::MachineId,
) -> FunctionSpillFrameRequirements {
    reconstruct_function(
        &omega_selected_instructions_to_register_homes::FunctionAbstractSpillAccessConstraints {
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
