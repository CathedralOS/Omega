//! Shared staged result and custody receipt for exact same-view-copy rules.

use crate::{Aarch64SameViewCopyElisionIdentity, ValidatedAarch64SameViewCopyElision};
use omega_optimization_core::{Optimization, OptimizationSelectionIdentity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedAarch64SameViewCopyElision {
    elision: ValidatedAarch64SameViewCopyElision,
    custody: StagedOptimizedAarch64SameViewCopyElisionCustodyReceipt,
}

impl StagedOptimizedAarch64SameViewCopyElision {
    pub(super) const fn new(
        elision: ValidatedAarch64SameViewCopyElision,
        custody: StagedOptimizedAarch64SameViewCopyElisionCustodyReceipt,
    ) -> Self {
        Self { elision, custody }
    }

    pub const fn elision(&self) -> &ValidatedAarch64SameViewCopyElision {
        &self.elision
    }

    pub const fn custody(&self) -> StagedOptimizedAarch64SameViewCopyElisionCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedAarch64SameViewCopyElisionCustodyReceipt {
    optimization: Optimization,
    selections: OptimizationSelectionIdentity,
    post_allocation_machine_selections: OptimizationSelectionIdentity,
    source: omega_physical_instructions::PostAllocationMachineIdentity,
    elision: Aarch64SameViewCopyElisionIdentity,
    action_count: usize,
}

impl StagedOptimizedAarch64SameViewCopyElisionCustodyReceipt {
    #[allow(clippy::too_many_arguments)]
    pub(super) const fn new(
        optimization: Optimization,
        selections: OptimizationSelectionIdentity,
        post_allocation_machine_selections: OptimizationSelectionIdentity,
        source: omega_physical_instructions::PostAllocationMachineIdentity,
        elision: Aarch64SameViewCopyElisionIdentity,
        action_count: usize,
    ) -> Self {
        Self {
            optimization,
            selections,
            post_allocation_machine_selections,
            source,
            elision,
            action_count,
        }
    }

    pub const fn optimization(self) -> Optimization {
        self.optimization
    }
    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }
    pub const fn post_allocation_machine_selections(self) -> OptimizationSelectionIdentity {
        self.post_allocation_machine_selections
    }
    pub const fn source(self) -> omega_physical_instructions::PostAllocationMachineIdentity {
        self.source
    }
    pub const fn elision(self) -> Aarch64SameViewCopyElisionIdentity {
        self.elision
    }
    pub const fn action_count(self) -> usize {
        self.action_count
    }
}
