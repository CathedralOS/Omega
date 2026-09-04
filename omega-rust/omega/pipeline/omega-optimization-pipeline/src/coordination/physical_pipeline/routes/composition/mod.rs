//! Optimizer module role: executable entrance. Exact physical-phase composition entrance.
//!
//! This is the sole route-policy join. Stage catalogs decide whether each
//! phase selection exists and applies to the target; this entrance decides
//! which validated phase sets may share one physical conveyor.

mod model;
#[cfg(test)]
mod tests;
use super::super::OptimizedVerifiedPhysicalPipelineError;
use super::super::PhysicalOptimizationPhaseSelections;
use crate::stages::layout::x86_branch_relaxation::x86_rel8_selected;
pub(crate) use model::{ResolvedPhysicalPhaseComposition, ResolvedRealizationPlan};
use omega_machine_optimizer::selected_post_allocation_machine_rule;
use omega_optimization_core::Optimization;
use omega_regalloc::{resolve_selected_lowering_rules, selected_allocation_recovery_rule};
use omega_target::Architecture;

pub(crate) fn resolve_physical_phase_composition(
    phases: &PhysicalOptimizationPhaseSelections,
    architecture: Architecture,
) -> Result<ResolvedPhysicalPhaseComposition, OptimizedVerifiedPhysicalPipelineError> {
    let selected_lowering_phase = phases.selected_lowering();
    let selected_lowering = selected_lowering_phase.selections();
    if !selected_lowering.is_empty() {
        resolve_selected_lowering_rules(selected_lowering_phase)
            .map_err(OptimizedVerifiedPhysicalPipelineError::SelectedLoweringRuleCatalog)?;
    }

    let allocation_recovery = selected_allocation_recovery_rule(phases.allocation_recovery())
        .map_err(OptimizedVerifiedPhysicalPipelineError::AllocationRecoveryRuleCatalog)?;
    let post_allocation_phase = phases.post_allocation_machine();
    let post_allocation = post_allocation_phase.selections();
    let function_relative_phase = phases.function_relative_layout();
    let function_relative = function_relative_phase.selections();

    if let Some(rule) = allocation_recovery {
        if !selected_lowering.is_empty() || !function_relative.is_empty() {
            return Err(
                OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition,
            );
        }
        let post_allocation = if post_allocation.is_empty() {
            None
        } else {
            let supported = post_allocation.as_slice()
                == [Optimization::X86SelectXorZeroI64MaterializationV1]
                || post_allocation.as_slice()
                    == [Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1]
                || post_allocation.as_slice()
                    == [Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1]
                || post_allocation.as_slice()
                    == [Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1];
            if rule != Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1
                || !supported
            {
                return Err(
                    OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition,
                );
            }
            Some(
                selected_post_allocation_machine_rule(post_allocation_phase, architecture)
                    .map_err(
                        OptimizedVerifiedPhysicalPipelineError::PostAllocationMachineRuleCatalog,
                    )?
                    .0,
            )
        };
        return Ok(ResolvedPhysicalPhaseComposition::AllocationRecovery {
            rule,
            post_allocation,
        });
    }

    if !post_allocation.is_empty() && !function_relative.is_empty() {
        return Err(OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition);
    }
    if !post_allocation.is_empty() {
        let (entry, _) = selected_post_allocation_machine_rule(post_allocation_phase, architecture)
            .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachineRuleCatalog)?;
        return Ok(ResolvedPhysicalPhaseComposition::Realization(
            ResolvedRealizationPlan::PostAllocationMachine { entry },
        ));
    }

    let function_relative_layout = x86_rel8_selected(function_relative_phase, architecture)
        .map_err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeLayoutRuleCatalog)?;
    let route = match (selected_lowering.is_empty(), function_relative_layout) {
        (true, false) => ResolvedRealizationPlan::Identity,
        (true, true) => ResolvedRealizationPlan::FunctionRelativeLayout,
        (false, _) => ResolvedRealizationPlan::SelectedLowering,
    };
    Ok(ResolvedPhysicalPhaseComposition::Realization(route))
}
