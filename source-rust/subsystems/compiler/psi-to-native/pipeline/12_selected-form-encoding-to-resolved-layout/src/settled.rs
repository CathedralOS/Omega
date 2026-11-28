pub use machine_code::layout::relaxation_policy::ResolvedLayoutOptimizationError;
use machine_code::layout::relaxation_policy::x86_rel8_selected;
#[cfg(test)]
use machine_code::layout::relaxation_policy::{
    FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG, FunctionRelativeLayoutCatalogError,
};
#[cfg(test)]
use optimization_core::Optimization;
use optimization_core::OptimizationExecutionPhase;
#[cfg(test)]
use target::Architecture;

use crate::x86_branch_relaxation;
use encoding_admission::layout::relaxation;
use machine_code::layout::X86BranchRelaxationRecord;

/// The final layout and the trace issued by the pass that produced it.
#[derive(Debug)]
pub struct ResolvedFunctionLayout<Contracts> {
    contracts: machine_code::layout::EncodedBodyContracts<Contracts>,
    layout: machine_code::OptimizedMachineLayout,
}

impl<Contracts> ResolvedFunctionLayout<Contracts> {
    /// Independent replay compares this stage's output with the captured encoding.
    pub fn layout_for_audit(&self) -> &machine_code::OptimizedMachineLayout {
        &self.layout
    }

    pub fn into_placement(
        self,
    ) -> Option<(
        machine_code::OptimizedMachineLayout,
        machine_code::layout::FramePlacementContracts<Contracts>,
    )> {
        let contracts = self.contracts.into_placement(&self.layout.program)?;
        Some((self.layout, contracts))
    }
}

pub(super) fn settle<Contracts>(
    mut program: machine_code::ResolvedMachineProgram,
    contracts: machine_code::layout::EncodedBodyContracts<Contracts>,
) -> Result<ResolvedFunctionLayout<Contracts>, ResolvedLayoutOptimizationError> {
    let selections = program
        .context
        .publication
        .selections()
        .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
    let budget = program.context.publication.budget();
    let enabled = x86_rel8_selected(&selections, program.layout.target().architecture)
        .map_err(ResolvedLayoutOptimizationError::Catalog)?;
    let relaxation = if enabled {
        Some(
            x86_branch_relaxation::stage_optimized_x86_branch_relaxation(
                program.context.physical(),
                &program.layout,
                &program.control_alternatives,
                budget,
            )
            .map_err(ResolvedLayoutOptimizationError::Relaxation)?,
        )
    } else {
        None
    };
    let baseline = program.layout.roots();
    if let Some(issued) = relaxation.as_ref().filter(|_| contracts.audit_requested()) {
        relaxation::validate_layout_transition(
            &program.layout,
            issued.layout(),
            &program.control_alternatives,
        )
        .map_err(ResolvedLayoutOptimizationError::Relaxation)?;
    }
    if let Some(issued) = &relaxation {
        program.layout = issued.shared_layout();
    }
    let layout = machine_code::OptimizedMachineLayout {
        baseline,
        program,
        selections,
        budget,
        relaxation: relaxation.as_ref().map(X86BranchRelaxationRecord::identity),
        trace: relaxation.map(std::sync::Arc::new),
    };
    if contracts.audit_requested() {
        relaxation::validate_current_layout_optimization(&layout)?;
    }
    Ok(ResolvedFunctionLayout { contracts, layout })
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
