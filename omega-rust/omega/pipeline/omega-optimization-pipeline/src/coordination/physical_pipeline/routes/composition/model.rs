use omega_machine_optimizer::PostAllocationMachineRuleCatalogEntry;
use omega_optimization_core::Optimization;

/// Exact physical route admitted for one canonical post-Terminal selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResolvedPhysicalPhaseComposition {
    AllocationRecovery {
        rule: Optimization,
        post_allocation: Option<PostAllocationMachineRuleCatalogEntry>,
    },
    Realization(ResolvedRealizationPlan),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResolvedRealizationPlan {
    Identity,
    SelectedLowering,
    PostAllocationMachine {
        entry: PostAllocationMachineRuleCatalogEntry,
    },
    FunctionRelativeLayout,
}
