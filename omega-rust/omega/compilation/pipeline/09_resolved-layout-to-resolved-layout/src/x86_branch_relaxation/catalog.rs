//! Exact function-relative-layout rule catalog.

use optimization_core::{
    Optimization, OptimizationCatalogDescriptor, OptimizationExecutionPhase,
    OptimizationPhaseMismatch, OptimizationPhaseSelections,
};
use target::Architecture;

pub type FunctionRelativeLayoutRuleCatalogEntry = OptimizationCatalogDescriptor<Architecture>;

/// The single function-relative-layout enable/order/applicability catalog.
pub const FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG: [FunctionRelativeLayoutRuleCatalogEntry; 1] =
    [FunctionRelativeLayoutRuleCatalogEntry::new(
        Optimization::X86RelaxConditionalBranchesToRel8V1,
        Architecture::X86_64,
    )];

pub const ORDERED_FUNCTION_RELATIVE_LAYOUT_RULES: [Optimization; 1] =
    [FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG[0].optimization()];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRelativeLayoutCatalogError {
    WrongPhase(OptimizationPhaseMismatch),
    UnsupportedSelection(Optimization),
    UnsupportedComposition,
    UnsupportedTarget {
        optimization: Optimization,
        required: Architecture,
        actual: Architecture,
    },
}

pub fn x86_rel8_selected(
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
    use super::{
        Architecture, FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG, FunctionRelativeLayoutCatalogError,
        ORDERED_FUNCTION_RELATIVE_LAYOUT_RULES, Optimization, OptimizationExecutionPhase,
        x86_rel8_selected,
    };
    use optimization_core::OptimizationSelections;

    #[test]
    fn ordered_catalog_covers_every_declared_function_relative_layout_optimization_once() {
        let declared = Optimization::ALL
            .into_iter()
            .filter(|optimization| {
                optimization.execution_phase() == OptimizationExecutionPhase::FunctionRelativeLayout
            })
            .collect::<Vec<_>>();
        assert_eq!(declared, ORDERED_FUNCTION_RELATIVE_LAYOUT_RULES);
        let selections =
            OptimizationSelections::new(ORDERED_FUNCTION_RELATIVE_LAYOUT_RULES).unwrap();
        let phase = selections.project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
        assert_eq!(
            FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG.map(|entry| entry.optimization()),
            ORDERED_FUNCTION_RELATIVE_LAYOUT_RULES,
        );
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
        // The empty phase selection is the disabled leg.
        let empty = OptimizationSelections::new([])
            .unwrap()
            .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
        assert_eq!(x86_rel8_selected(&empty, Architecture::X86_64), Ok(false));

        // A selection naming only another phase's rules projects into this
        // phase as empty: foreign rules cannot name this rule's enable bit.
        let foreign =
            OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate])
                .unwrap()
                .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
        assert_eq!(x86_rel8_selected(&foreign, Architecture::X86_64), Ok(false));

        // A mixed selection routes only this phase's member here; the
        // SelectedLowering member is consumed by its own stage.
        let mixed = OptimizationSelections::new([
            Optimization::X86RelaxConditionalBranchesToRel8V1,
            Optimization::SelectedIncomingU12ExactAddImmediate,
        ])
        .unwrap()
        .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
        assert_eq!(x86_rel8_selected(&mixed, Architecture::X86_64), Ok(true));

        // A projection built for another phase is rejected at the entrance
        // even when this rule was in the complete selection.
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
