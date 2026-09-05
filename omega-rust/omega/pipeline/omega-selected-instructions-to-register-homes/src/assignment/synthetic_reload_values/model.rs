use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::{RegisterClassId, RegisterViewId};
use omega_selected_instructions::SelectedBlockId;
use psi_core::MachineId;

use crate::{
    AbstractSpillInsertionIdentity, LiveRangePoint, LogicalReloadValueId, ReloadValueHomeIdentity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SyntheticReloadValuePlanIdentity(pub(crate) [u8; 32]);

impl SyntheticReloadValuePlanIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// A compiler-private namespace distinct from selected virtual-register IDs.
/// The epoch field reserves an explicit rung for bounded recursive recovery;
/// V1 admits only the already validated first epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SyntheticReloadValueId {
    pub epoch: u32,
    pub ordinal: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntheticReloadValuePolicy {
    ValidatedSingleSpillEpochZeroCanonicalOrderV1,
}

/// Namespace custody only. This plan is not a selected-plan rewrite and does
/// not authorize spill-pseudo, memory, frame, trap, encoding, or publication
/// construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticReloadValuePlan {
    pub abstract_spill_insertion: AbstractSpillInsertionIdentity,
    pub reload_value_homes: ReloadValueHomeIdentity,
    pub policy: SyntheticReloadValuePolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionSyntheticReloadValues>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSyntheticReloadValues {
    pub machine: MachineId,
    pub binding: Option<SyntheticReloadValueBinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntheticReloadValueBinding {
    pub logical: LogicalReloadValueId,
    pub synthetic: SyntheticReloadValueId,
    pub block: SelectedBlockId,
    pub start: LiveRangePoint,
    pub exclusive_end: LiveRangePoint,
    pub class: RegisterClassId,
    pub view: RegisterViewId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntheticReloadValueReceipt {
    pub(crate) identity: SyntheticReloadValuePlanIdentity,
    pub(crate) abstract_spill_insertion: AbstractSpillInsertionIdentity,
    pub(crate) reload_value_homes: ReloadValueHomeIdentity,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) binding_count: usize,
}

impl SyntheticReloadValueReceipt {
    pub const fn identity(self) -> SyntheticReloadValuePlanIdentity {
        self.identity
    }

    pub const fn abstract_spill_insertion(self) -> AbstractSpillInsertionIdentity {
        self.abstract_spill_insertion
    }

    pub const fn reload_value_homes(self) -> ReloadValueHomeIdentity {
        self.reload_value_homes
    }

    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }

    pub const fn function_count(self) -> usize {
        self.function_count
    }

    pub const fn binding_count(self) -> usize {
        self.binding_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedSyntheticReloadValues {
    pub(crate) plan: SyntheticReloadValuePlan,
    pub(crate) receipt: SyntheticReloadValueReceipt,
}

impl ValidatedSyntheticReloadValues {
    pub const fn plan(&self) -> &SyntheticReloadValuePlan {
        &self.plan
    }

    pub const fn receipt(&self) -> SyntheticReloadValueReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyntheticReloadValueError {
    RootMismatch,
    UnsupportedPolicy,
    FunctionMismatch {
        function: usize,
    },
    MissingReloadHome {
        function: usize,
    },
    UnexpectedReloadHome {
        function: usize,
    },
    ReloadMismatch {
        function: usize,
    },
    SyntheticNamespaceOverflow,
    NonCanonicalNamespace {
        function: usize,
    },
    WorkOverflow,
    UsageMismatch,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
}

impl std::fmt::Display for SyntheticReloadValueError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "synthetic reload-value binding failed: {self:?}")
    }
}

impl std::error::Error for SyntheticReloadValueError {}
