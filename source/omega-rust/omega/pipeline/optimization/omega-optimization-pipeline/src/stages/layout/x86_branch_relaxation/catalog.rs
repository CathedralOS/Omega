//! Exact function-relative-layout rule catalog.

use omega_optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};

/// Canonical function-relative-layout rule order.
pub const ORDERED_FUNCTION_RELATIVE_LAYOUT_RULES: [Optimization; 1] =
    [Optimization::X86RelaxConditionalBranchesToRel8V1];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FunctionRelativeLayoutCatalogError {
    UnsupportedSelection(Optimization),
    UnsupportedComposition,
}

pub(crate) fn x86_rel8_selected(
    selections: &OptimizationSelections,
) -> Result<bool, FunctionRelativeLayoutCatalogError> {
    let phase = selections.for_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
    match phase.as_slice() {
        [] => Ok(false),
        [Optimization::X86RelaxConditionalBranchesToRel8V1] => Ok(true),
        [unsupported] => Err(FunctionRelativeLayoutCatalogError::UnsupportedSelection(
            *unsupported,
        )),
        _ => Err(FunctionRelativeLayoutCatalogError::UnsupportedComposition),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(x86_rel8_selected(&selections), Ok(true));
    }
}
