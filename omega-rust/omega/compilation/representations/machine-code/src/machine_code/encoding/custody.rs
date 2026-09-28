//! Non-authorizing content joins for separately validated machine rewrites.

use optimization_core::OptimizationSelectionIdentity;
use physical_instructions::{Aarch64CbnzFusionIdentity, Aarch64MovnMaterializationIdentity};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SelectedFormMachineOptimizationCustody {
    pub(super) selections: OptimizationSelectionIdentity,
    pub(super) post_allocation_machine_selections: OptimizationSelectionIdentity,
    pub(super) fusion: Aarch64CbnzFusionIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SelectedFormMovnOptimizationCustody {
    pub(super) selections: OptimizationSelectionIdentity,
    pub(super) post_allocation_machine_selections: OptimizationSelectionIdentity,
    pub(super) materialization: Aarch64MovnMaterializationIdentity,
}

impl SelectedFormMovnOptimizationCustody {
    /// Reconstruct custody decoded from an independently validated layout artifact.
    pub const fn from_parts(
        selections: OptimizationSelectionIdentity,
        post_allocation_machine_selections: OptimizationSelectionIdentity,
        materialization: Aarch64MovnMaterializationIdentity,
    ) -> Self {
        Self {
            selections,
            post_allocation_machine_selections,
            materialization,
        }
    }

    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }

    pub const fn post_allocation_machine_selections(self) -> OptimizationSelectionIdentity {
        self.post_allocation_machine_selections
    }

    pub const fn materialization(self) -> Aarch64MovnMaterializationIdentity {
        self.materialization
    }
}

impl SelectedFormMachineOptimizationCustody {
    /// Reconstruct custody decoded from an independently validated layout artifact.
    pub const fn from_parts(
        selections: OptimizationSelectionIdentity,
        post_allocation_machine_selections: OptimizationSelectionIdentity,
        fusion: Aarch64CbnzFusionIdentity,
    ) -> Self {
        Self {
            selections,
            post_allocation_machine_selections,
            fusion,
        }
    }

    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }

    pub const fn post_allocation_machine_selections(self) -> OptimizationSelectionIdentity {
        self.post_allocation_machine_selections
    }

    pub const fn fusion(self) -> Aarch64CbnzFusionIdentity {
        self.fusion
    }
}
