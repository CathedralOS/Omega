//! Direct traversal over authenticated abstract access rows.

use crate::ValidatedTargetRegisterEnvironment;
use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_regalloc::ValidatedAbstractSpillAccessConstraints;

use super::{
    FrameAbiPreservationConvention, FunctionSpillFrameRequirements,
    NonAuthoritativeSpillFrameRequirementPlan, NonAuthoritativeSpillFrameRequirementPolicy,
    SpillFrameRequirementError,
};

pub(super) fn derive(
    source: &ValidatedAbstractSpillAccessConstraints,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: NonAuthoritativeSpillFrameRequirementPolicy,
    budget: OptimizationWorkBudget,
) -> Result<NonAuthoritativeSpillFrameRequirementPlan, SpillFrameRequirementError> {
    if policy
        != NonAuthoritativeSpillFrameRequirementPolicy::AbstractSpillAreaAndPreservationConventionV1
    {
        return Err(SpillFrameRequirementError::UnsupportedPolicy);
    }
    if source.receipt().register_environment() != environment.identity() {
        return Err(SpillFrameRequirementError::RootMismatch);
    }
    let target = environment.target();
    let (abi_preservation_convention, convention) =
        match (target.architecture, target.object_format) {
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
    let functions = source
        .plan()
        .functions
        .iter()
        .map(|function| {
            derive_function(
                function,
                abi_preservation_convention,
                convention.stack_alignment,
                convention.red_zone_bytes,
            )
        })
        .collect::<Vec<_>>();
    let function_count = count(functions.len())?;
    let placement_count = source
        .plan()
        .functions
        .iter()
        .try_fold(0_u64, |total, function| {
            total
                .checked_add(count(function.placements.len())?)
                .ok_or(SpillFrameRequirementError::WorkOverflow)
        })?;
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
    Ok(NonAuthoritativeSpillFrameRequirementPlan {
        abstract_spill_access_constraints: source.receipt().identity(),
        register_environment: environment.identity(),
        target: environment.target(),
        policy,
        budget,
        usage,
        functions,
    })
}

fn count(value: usize) -> Result<u64, SpillFrameRequirementError> {
    u64::try_from(value).map_err(|_| SpillFrameRequirementError::WorkOverflow)
}

fn derive_function(
    function: &omega_regalloc::FunctionAbstractSpillAccessConstraints,
    abi: FrameAbiPreservationConvention,
    stack_alignment: u16,
    red_zone_capacity_bytes: u16,
) -> FunctionSpillFrameRequirements {
    FunctionSpillFrameRequirements {
        machine: function.machine,
        abstract_spill_area_bytes: function.spill_area_bytes,
        abstract_spill_area_alignment: function
            .placements
            .iter()
            .map(|placement| placement.alignment_bytes)
            .max()
            .unwrap_or(1),
        abi_preservation_convention: abi,
        abi_stack_alignment: stack_alignment,
        abi_red_zone_capacity_bytes: red_zone_capacity_bytes,
    }
}

#[cfg(test)]
pub(crate) fn derive_zero_access_requirement_for_test(
    machine: psi_core::MachineId,
) -> FunctionSpillFrameRequirements {
    derive_function(
        &omega_regalloc::FunctionAbstractSpillAccessConstraints {
            machine,
            spill_area_bytes: 0,
            placements: Vec::new(),
            dependencies: Vec::new(),
        },
        FrameAbiPreservationConvention::SystemVAMD64,
        16,
        128,
    )
}
