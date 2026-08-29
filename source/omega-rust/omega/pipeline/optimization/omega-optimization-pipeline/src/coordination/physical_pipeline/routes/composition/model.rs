use omega_optimization_core::Optimization;

/// Exact physical route admitted for one canonical optimization selection.
/// Psi selections are orthogonal and remain on every route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResolvedPhysicalPhaseComposition {
    AllocationRecovery { rule: Optimization },
    NonAllocation(ResolvedNonAllocationComposition),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResolvedNonAllocationComposition {
    Baseline,
    SelectedLowering,
    SelectedLoweringWithFunctionRelativeLayout,
    PostAllocationMachine {
        rule: Optimization,
        after_selected_lowering: bool,
    },
    FunctionRelativeLayout,
}
