use omega_optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};

/// Canonical post-allocation rule order. This is the stage's only registry;
/// rule leaves own mechanics, while complete compiler routes consume the
/// selected entry through the stage entrance.
pub const ORDERED_RULES: [Optimization; 3] = [
    Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
    Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
    Optimization::X86SelectXorZeroI64MaterializationV1,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostAllocationMachineCatalogError {
    MissingSelection,
    UnsupportedSelection(Optimization),
    UnsupportedComposition(Optimization),
}

pub fn selected_rule(
    selections: &OptimizationSelections,
) -> Result<(Optimization, OptimizationSelections), PostAllocationMachineCatalogError> {
    let phase = selections.for_phase(OptimizationExecutionPhase::PostAllocationMachine);
    match phase.as_slice() {
        [selected] if ORDERED_RULES.contains(selected) => Ok((*selected, phase)),
        [] => Err(PostAllocationMachineCatalogError::MissingSelection),
        [selected] => Err(PostAllocationMachineCatalogError::UnsupportedSelection(
            *selected,
        )),
        selected => Err(PostAllocationMachineCatalogError::UnsupportedComposition(
            selected[0],
        )),
    }
}

pub fn require_rule(
    selections: &OptimizationSelections,
    expected: Optimization,
) -> Result<OptimizationSelections, PostAllocationMachineCatalogError> {
    let (selected, phase) = selected_rule(selections)?;
    if selected != expected {
        return Err(PostAllocationMachineCatalogError::UnsupportedSelection(
            selected,
        ));
    }
    Ok(phase)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_exactly_matches_the_post_allocation_vocabulary() {
        let declared = Optimization::ALL
            .into_iter()
            .filter(|optimization| {
                optimization.execution_phase() == OptimizationExecutionPhase::PostAllocationMachine
            })
            .collect::<Vec<_>>();
        assert_eq!(declared, ORDERED_RULES);
        for optimization in ORDERED_RULES {
            let selections = OptimizationSelections::new([optimization]).unwrap();
            let (scheduled, phase) = selected_rule(&selections).unwrap();
            assert_eq!(scheduled, optimization);
            assert_eq!(phase, selections);
        }
        assert!(matches!(
            selected_rule(&OptimizationSelections::new(ORDERED_RULES).unwrap()),
            Err(PostAllocationMachineCatalogError::UnsupportedComposition(_))
        ));
    }
}
