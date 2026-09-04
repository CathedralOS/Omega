use omega_machine_optimizer::PostAllocationMachineRuleCatalogEntry;
use omega_optimization_core::Optimization;

/// Exact physical route admitted for one canonical optimization selection.
/// Psi selections are orthogonal and remain on every route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResolvedPhysicalPhaseComposition {
    AllocationRecovery {
        rule: Optimization,
        post_allocation: Option<PostAllocationMachineRuleCatalogEntry>,
    },
    NonAllocation(ResolvedNonAllocationComposition),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResolvedNonAllocationComposition {
    Identity,
    SelectedLowering,
    SelectedLoweringWithFunctionRelativeLayout,
    PostAllocationMachine {
        entry: PostAllocationMachineRuleCatalogEntry,
        after_selected_lowering: bool,
    },
    FunctionRelativeLayout,
}
