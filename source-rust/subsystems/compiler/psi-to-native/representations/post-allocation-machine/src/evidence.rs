// Publication needs to know which optimization selections the post-allocation
// phase ran. The rewritten plan itself travels beside this receipt, so the
// receipt names only the selections, not a hash of the result.

use optimization_core::OptimizationSelectionIdentity;

/// The completed phase roots publication needs. The optimizer's action history
/// stays with explicit replay, not the ordinary physical program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostAllocationMachinePhaseReceipt {
    complete_selection: OptimizationSelectionIdentity,
    phase_selections: OptimizationSelectionIdentity,
}

impl PostAllocationMachinePhaseReceipt {
    pub const fn from_parts(
        complete_selection: OptimizationSelectionIdentity,
        phase_selections: OptimizationSelectionIdentity,
    ) -> Self {
        Self {
            complete_selection,
            phase_selections,
        }
    }

    pub const fn complete_selection(self) -> OptimizationSelectionIdentity {
        self.complete_selection
    }

    pub const fn phase_selections(self) -> OptimizationSelectionIdentity {
        self.phase_selections
    }
}
