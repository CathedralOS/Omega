/*
We execute and validate function-relative layout optimizations (`ResolvedLayoutOptimization`) for Omega stage 09 (`resolved-layout-to-resolved-layout`). Starting from the validated physical register model, the baseline `ResolvedMachineLayout` produced in stage 08, the phase optimization selections, and the work budget, `execute_resolved_layout_optimization` consults `FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG` (`x86_rel8_selected`) to determine whether x86-64 conditional-branch relaxation (`Optimization::X86RelaxConditionalBranchesToRel8V1`) is selected. When enabled on an x86-64 target, we run the monotone batched fixed-point sweep in `x86_branch_relaxation` to shrink eligible six-byte near conditional branches (`Jcc rel32`) into two-byte short conditional branches (`Jcc rel8`) and reflow all function-relative offsets and branch displacements; when disabled, we preserve the baseline layout unchanged.
*/

/*
Even when no function-relative layout rewrite is enabled, routing the baseline layout through an explicit X-to-X optimization phase keeps downstream emission decoupled from whether optional layout rewrites ran. Because narrowing a branch reduces byte offsets and only ever brings surrounding branch displacements closer to zero, batched relaxation sweeps over a finite branch roster are monotone and converge on the same least fixed point as one-at-a-time commits while requiring far fewer full-function offset reflows. Recording the attempt and action trace alongside the rewritten `ResolvedMachineLayout` lets `validate_resolved_layout_optimization` verify phase-selection routing, work-budget compliance, and root identity custody without re-running the fixed-point search during normal compilation.
*/

/*
- Entrypoints:
  - `execute_resolved_layout_optimization`: checks the function-relative layout catalog and either runs x86-64 short-branch relaxation or forwards the baseline layout.
  - `validate_resolved_layout_optimization`: verifies that the artifact's phase selections, relaxation trace roots, work budget, and current layout agree with the baseline layout.
- Custody:
  - `ResolvedLayoutOptimization`: holds the current `Arc<ResolvedMachineLayout>`, the phase selections, the work budget, and the optional `StagedOptimizedX86BranchRelaxation` evidence carrier.
  - `ResolvedLayoutOptimizationError` and `FunctionRelativeLayoutCatalogError`: report catalog phase mismatches, unsupported targets, relaxation failures, or current-program mismatches.
- Support modules:
  - `x86_branch_relaxation`: owns the x86-64 `Jcc rel32` to `Jcc rel8` fixed-point sweep, predicate encoding/validation lanes, dense offset reflow, and work-budget accounting.
*/

#![forbid(unsafe_code)]

use std::sync::Arc;

use machine_code::ResolvedMachineLayout;
use optimization_core::{
    Optimization, OptimizationCatalogDescriptor, OptimizationExecutionPhase,
    OptimizationPhaseMismatch, OptimizationPhaseSelections, OptimizationWorkBudget,
};
use register_model::ValidatedPhysicalRegisterModel;
use target::Architecture;

mod x86_branch_relaxation;

pub use x86_branch_relaxation::{
    OptimizedX86BranchRelaxationError, StagedOptimizedX86BranchRelaxation,
    X86BranchRelaxationAction, X86BranchRelaxationAttempt, X86BranchRelaxationAttemptOutcome,
    X86BranchRelaxationIdentity, X86BranchRelaxationPolicy, X86BranchRelaxationRevisionIdentity,
};
use x86_branch_relaxation::{
    stage_optimized_x86_branch_relaxation, validate_optimized_x86_branch_relaxation,
};

pub(crate) type FunctionRelativeLayoutRuleCatalogEntry =
    OptimizationCatalogDescriptor<Architecture>;

pub(crate) const FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG: [FunctionRelativeLayoutRuleCatalogEntry;
    1] = [FunctionRelativeLayoutRuleCatalogEntry::new(
    Optimization::X86RelaxConditionalBranchesToRel8V1,
    Architecture::X86_64,
)];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRelativeLayoutCatalogError {
    WrongPhase(OptimizationPhaseMismatch),
    UnsupportedSelection(Optimization),
    UnsupportedComposition,
    UnsupportedTarget {
        optimization: Optimization, // Optimization rule rejected for the target architecture.
        required: Architecture,     // Architecture required by the catalog entry.
        actual: Architecture,       // Architecture supplied by the compilation target.
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedLayoutOptimizationError {
    Catalog(FunctionRelativeLayoutCatalogError),
    Relaxation(OptimizedX86BranchRelaxationError),
    CurrentProgramMismatch,
    SelectionMismatch,
}

impl std::fmt::Display for ResolvedLayoutOptimizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "resolved-layout optimization failed: {self:?}")
    }
}

impl std::error::Error for ResolvedLayoutOptimizationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLayoutOptimization {
    current: Arc<ResolvedMachineLayout>, // Active resolved machine layout after optional relaxation.
    selections: OptimizationPhaseSelections, // Phase-projected optimization selections governing this run.
    budget: OptimizationWorkBudget,          // Work budget enforced across relaxation sweeps.
    relaxation: Option<StagedOptimizedX86BranchRelaxation>, // Retained x86-64 branch-relaxation trace when enabled.
}

impl ResolvedLayoutOptimization {
    pub fn layout(&self) -> &ResolvedMachineLayout {
        &self.current
    }

    pub fn shared_layout(&self) -> Arc<ResolvedMachineLayout> {
        Arc::clone(&self.current)
    }

    pub fn relaxation(&self) -> Option<&StagedOptimizedX86BranchRelaxation> {
        self.relaxation.as_ref()
    }

    pub fn selections(&self) -> &OptimizationPhaseSelections {
        &self.selections
    }

    pub const fn budget(&self) -> OptimizationWorkBudget {
        self.budget
    }
}

#[allow(clippy::too_many_arguments)]
pub fn execute_resolved_layout_optimization(
    physical: &ValidatedPhysicalRegisterModel,
    baseline: &Arc<ResolvedMachineLayout>,
    selections: &OptimizationPhaseSelections,
    budget: OptimizationWorkBudget,
) -> Result<ResolvedLayoutOptimization, ResolvedLayoutOptimizationError> {
    let enabled = x86_rel8_selected(selections, baseline.target().architecture)
        .map_err(ResolvedLayoutOptimizationError::Catalog)?;
    let relaxation = if enabled {
        Some(
            stage_optimized_x86_branch_relaxation(physical, baseline, budget)
                .map_err(ResolvedLayoutOptimizationError::Relaxation)?,
        )
    } else {
        None
    };
    let current = match &relaxation {
        Some(relaxation) => relaxation.shared_layout(),
        None => Arc::clone(baseline),
    };
    Ok(ResolvedLayoutOptimization {
        current,
        selections: selections.clone(),
        budget,
        relaxation,
    })
}

pub fn validate_resolved_layout_optimization(
    physical: &ValidatedPhysicalRegisterModel,
    baseline: &ResolvedMachineLayout,
    selections: &OptimizationPhaseSelections,
    artifact: &ResolvedLayoutOptimization,
) -> Result<(), ResolvedLayoutOptimizationError> {
    if &artifact.selections != selections {
        return Err(ResolvedLayoutOptimizationError::SelectionMismatch);
    }
    let enabled = x86_rel8_selected(selections, baseline.target().architecture)
        .map_err(ResolvedLayoutOptimizationError::Catalog)?;
    match (enabled, artifact.relaxation.as_ref()) {
        (false, None) => {
            if artifact.layout() != baseline {
                return Err(ResolvedLayoutOptimizationError::CurrentProgramMismatch);
            }
        }
        (true, Some(relaxation)) => {
            validate_optimized_x86_branch_relaxation(physical, baseline, relaxation)
                .map_err(ResolvedLayoutOptimizationError::Relaxation)?;
            if artifact.budget != relaxation.budget() || artifact.layout() != relaxation.layout() {
                return Err(ResolvedLayoutOptimizationError::CurrentProgramMismatch);
            }
        }
        _ => return Err(ResolvedLayoutOptimizationError::SelectionMismatch),
    }
    Ok(())
}

pub(crate) fn x86_rel8_selected(
    selections: &OptimizationPhaseSelections,
    architecture: Architecture,
) -> Result<bool, FunctionRelativeLayoutCatalogError> {
    let phase = selections
        .require_phase(OptimizationExecutionPhase::FunctionRelativeLayout)
        .map_err(FunctionRelativeLayoutCatalogError::WrongPhase)?;
    match phase.as_slice() {
        [] => Ok(false),
        [Optimization::X86RelaxConditionalBranchesToRel8V1] => {
            let descriptor = FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG[0];
            let required = *descriptor.payload();
            if architecture != required {
                return Err(FunctionRelativeLayoutCatalogError::UnsupportedTarget {
                    optimization: descriptor.optimization(),
                    required,
                    actual: architecture,
                });
            }
            Ok(true)
        }
        [unsupported] => Err(FunctionRelativeLayoutCatalogError::UnsupportedSelection(
            *unsupported,
        )),
        _ => Err(FunctionRelativeLayoutCatalogError::UnsupportedComposition),
    }
}

#[cfg(test)]
mod tests {
    use optimization_core::OptimizationSelections;

    use super::{
        Architecture, FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG, FunctionRelativeLayoutCatalogError,
        Optimization, OptimizationExecutionPhase, x86_rel8_selected,
    };

    #[test]
    fn ordered_catalog_covers_every_declared_function_relative_layout_optimization_once() {
        let declared = Optimization::ALL
            .into_iter()
            .filter(|optimization| {
                optimization.execution_phase() == OptimizationExecutionPhase::FunctionRelativeLayout
            })
            .collect::<Vec<_>>();
        assert_eq!(
            declared,
            FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG.map(|entry| entry.optimization())
        );
        let selections = OptimizationSelections::new(
            FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG.map(|entry| entry.optimization()),
        )
        .unwrap();
        let phase = selections.project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
        assert_eq!(x86_rel8_selected(&phase, Architecture::X86_64), Ok(true));
        assert_eq!(
            x86_rel8_selected(&phase, Architecture::Aarch64),
            Err(FunctionRelativeLayoutCatalogError::UnsupportedTarget {
                optimization: Optimization::X86RelaxConditionalBranchesToRel8V1,
                required: Architecture::X86_64,
                actual: Architecture::Aarch64,
            })
        );
    }

    #[test]
    fn selection_vocabulary_routes_phases_and_rejects_foreign_phase_projections() {
        let empty = OptimizationSelections::new([])
            .unwrap()
            .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
        assert_eq!(x86_rel8_selected(&empty, Architecture::X86_64), Ok(false));

        let foreign =
            OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate])
                .unwrap()
                .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
        assert_eq!(x86_rel8_selected(&foreign, Architecture::X86_64), Ok(false));

        let mixed = OptimizationSelections::new([
            Optimization::X86RelaxConditionalBranchesToRel8V1,
            Optimization::SelectedIncomingU12ExactAddImmediate,
        ])
        .unwrap()
        .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
        assert_eq!(x86_rel8_selected(&mixed, Architecture::X86_64), Ok(true));

        let wrong_phase =
            OptimizationSelections::new([Optimization::X86RelaxConditionalBranchesToRel8V1])
                .unwrap()
                .project_phase(OptimizationExecutionPhase::SelectedLowering);
        assert_eq!(
            x86_rel8_selected(&wrong_phase, Architecture::X86_64),
            Err(FunctionRelativeLayoutCatalogError::WrongPhase(
                optimization_core::OptimizationPhaseMismatch {
                    expected: OptimizationExecutionPhase::FunctionRelativeLayout,
                    actual: OptimizationExecutionPhase::SelectedLowering,
                },
            ))
        );
    }
}
