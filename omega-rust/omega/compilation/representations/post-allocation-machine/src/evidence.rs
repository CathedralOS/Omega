// We retain rule-independent post-allocation optimization custody metadata so
// machine encoding, code layout, and realization can bind optimization replay
// authority without growing one optional field per optimization rule.

use optimization_core::{Optimization, OptimizationSelectionIdentity};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PostAllocationMachineOptimizationCustody {
    optimization: Optimization,
    artifact_identity: [u8; 32],
    selections: OptimizationSelectionIdentity,
    post_allocation_machine_selections: OptimizationSelectionIdentity,
    source: crate::PostAllocationMachineIdentity,
    action_count: usize,
    baseline_bytes: u64,
    selected_bytes: u64,
}

impl PostAllocationMachineOptimizationCustody {
    #[allow(clippy::too_many_arguments)]
    pub const fn from_parts(
        optimization: Optimization,
        artifact_identity: [u8; 32],
        selections: OptimizationSelectionIdentity,
        post_allocation_machine_selections: OptimizationSelectionIdentity,
        source: crate::PostAllocationMachineIdentity,
        action_count: usize,
        baseline_bytes: u64,
        selected_bytes: u64,
    ) -> Self {
        Self {
            optimization,
            artifact_identity,
            selections,
            post_allocation_machine_selections,
            source,
            action_count,
            baseline_bytes,
            selected_bytes,
        }
    }

    pub const fn optimization(self) -> Optimization {
        self.optimization
    }

    pub const fn artifact_identity(self) -> [u8; 32] {
        self.artifact_identity
    }

    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }

    pub const fn post_allocation_machine_selections(self) -> OptimizationSelectionIdentity {
        self.post_allocation_machine_selections
    }

    pub const fn source(self) -> crate::PostAllocationMachineIdentity {
        self.source
    }

    pub const fn action_count(self) -> usize {
        self.action_count
    }

    pub const fn baseline_bytes(self) -> u64 {
        self.baseline_bytes
    }

    pub const fn selected_bytes(self) -> u64 {
        self.selected_bytes
    }
}
