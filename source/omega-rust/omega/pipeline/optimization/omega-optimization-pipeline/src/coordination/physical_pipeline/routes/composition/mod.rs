//! Optimizer module role: executable entrance. Exact physical-phase composition entrance.
//!
//! This is the sole route-policy join. Stage catalogs decide whether each
//! phase selection exists and applies to the target; this entrance decides
//! which validated phase sets may share one physical conveyor.

mod model;
#[cfg(test)]
mod tests;

use omega_machine_optimizer::selected_post_allocation_machine_rule;
use omega_optimization_core::{OptimizationExecutionPhase, OptimizationSelections};
use omega_regalloc::{resolve_selected_lowering_rules, selected_allocation_recovery_rule};
use omega_target::Architecture;

use crate::stages::layout::x86_branch_relaxation::x86_rel8_selected;

use super::super::OptimizedVerifiedPhysicalPipelineError;
pub(crate) use model::{ResolvedNonAllocationComposition, ResolvedPhysicalPhaseComposition};

pub(crate) fn resolve_physical_phase_composition(
    selections: &OptimizationSelections,
    architecture: Architecture,
) -> Result<ResolvedPhysicalPhaseComposition, OptimizedVerifiedPhysicalPipelineError> {
    let selected_lowering = selections.for_phase(OptimizationExecutionPhase::SelectedLowering);
    if !selected_lowering.is_empty() {
        resolve_selected_lowering_rules(selections)
            .map_err(OptimizedVerifiedPhysicalPipelineError::SelectedLoweringRuleCatalog)?;
    }

    let allocation_recovery = selected_allocation_recovery_rule(selections)
        .map_err(OptimizedVerifiedPhysicalPipelineError::AllocationRecoveryRuleCatalog)?;
    let post_allocation = selections.for_phase(OptimizationExecutionPhase::PostAllocationMachine);
    let function_relative =
        selections.for_phase(OptimizationExecutionPhase::FunctionRelativeLayout);

    if let Some(rule) = allocation_recovery {
        if !selected_lowering.is_empty()
            || !post_allocation.is_empty()
            || !function_relative.is_empty()
        {
            return Err(
                OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition,
            );
        }
        return Ok(ResolvedPhysicalPhaseComposition::AllocationRecovery { rule });
    }

    if !post_allocation.is_empty() && !function_relative.is_empty() {
        return Err(OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition);
    }
    if !post_allocation.is_empty() {
        let (entry, _) = selected_post_allocation_machine_rule(selections, architecture)
            .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachineRuleCatalog)?;
        return Ok(ResolvedPhysicalPhaseComposition::NonAllocation(
            ResolvedNonAllocationComposition::PostAllocationMachine {
                entry,
                after_selected_lowering: !selected_lowering.is_empty(),
            },
        ));
    }

    let function_relative_layout = x86_rel8_selected(selections, architecture)
        .map_err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeLayoutRuleCatalog)?;
    let route = match (selected_lowering.is_empty(), function_relative_layout) {
        (true, false) => ResolvedNonAllocationComposition::Baseline,
        (true, true) => ResolvedNonAllocationComposition::FunctionRelativeLayout,
        (false, false) => ResolvedNonAllocationComposition::SelectedLowering,
        (false, true) => {
            ResolvedNonAllocationComposition::SelectedLoweringWithFunctionRelativeLayout
        }
    };
    Ok(ResolvedPhysicalPhaseComposition::NonAllocation(route))
}
