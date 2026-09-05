use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use omega_selected_instructions::{SelectedBlockId, VirtualRegisterId};
use psi_core::MachineId;

use crate::{
    AbstractSpillInsertionIdentity, AllocationLegalityIdentity, AllocatorAvailabilityIdentity,
    LiveRangeIdentity, LiveRangePoint, LogicalReloadValueId, LogicalSpillOperationIdentity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReloadValueHomeIdentity(pub(crate) [u8; 32]);

impl ReloadValueHomeIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Exact bounded lane: replay the original linear-scan prefix, apply the
/// validated single spill, introduce the logical reload before its first
/// rewrite, and choose the lowest compatible view through its rewrite suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReloadValueHomePolicy {
    BlockLocalSingleSpillReloadFirstLowestCompatibleViewV1,
}

/// Reanalysis and physical-view assignment for logical reload values only.
/// This is not a selected or machine instruction plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReloadValueHomePlan {
    pub abstract_spill_insertion: AbstractSpillInsertionIdentity,
    pub logical_spill_operations: LogicalSpillOperationIdentity,
    pub legality: AllocationLegalityIdentity,
    pub ranges: LiveRangeIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub policy: ReloadValueHomePolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionReloadValueHomes>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionReloadValueHomes {
    pub machine: MachineId,
    pub assignment: Option<ReloadValueHomeAssignment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReloadValueHomeAssignment {
    pub result: LogicalReloadValueId,
    pub block: SelectedBlockId,
    pub start: LiveRangePoint,
    pub exclusive_end: LiveRangePoint,
    pub class: RegisterClassId,
    /// Intersection of physical views legal at every point of the synthetic
    /// reload lifetime, in canonical view-ID order.
    pub candidates: Vec<RegisterViewId>,
    /// Lowest candidate compatible with every reconstructed coexisting home.
    pub view: RegisterViewId,
    /// Canonical VReg-ID-sorted homes whose lifetimes overlap the reload.
    pub coexisting_homes: Vec<ReloadCoexistingHome>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReloadCoexistingHome {
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    pub view: RegisterViewId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReloadValueHomeReceipt {
    pub(crate) identity: ReloadValueHomeIdentity,
    pub(crate) abstract_spill_insertion: AbstractSpillInsertionIdentity,
    pub(crate) logical_spill_operations: LogicalSpillOperationIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) assignment_count: usize,
    pub(crate) coexisting_home_count: usize,
}

impl ReloadValueHomeReceipt {
    pub const fn identity(self) -> ReloadValueHomeIdentity {
        self.identity
    }
    pub const fn abstract_spill_insertion(self) -> AbstractSpillInsertionIdentity {
        self.abstract_spill_insertion
    }
    pub const fn logical_spill_operations(self) -> LogicalSpillOperationIdentity {
        self.logical_spill_operations
    }
    pub const fn legality(self) -> AllocationLegalityIdentity {
        self.legality
    }
    pub const fn ranges(self) -> LiveRangeIdentity {
        self.ranges
    }
    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn allocator_availability(self) -> AllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn assignment_count(self) -> usize {
        self.assignment_count
    }
    pub const fn coexisting_home_count(self) -> usize {
        self.coexisting_home_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedReloadValueHomes {
    pub(crate) plan: ReloadValueHomePlan,
    pub(crate) receipt: ReloadValueHomeReceipt,
}

impl ValidatedReloadValueHomes {
    pub const fn plan(&self) -> &ReloadValueHomePlan {
        &self.plan
    }
    pub const fn receipt(&self) -> ReloadValueHomeReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReloadValueHomeError {
    RootMismatch,
    UnsupportedPolicy,
    FunctionMismatch {
        function: usize,
    },
    UnsupportedConstraintTopology {
        function: usize,
    },
    UnsupportedReloadShape {
        function: usize,
    },
    VirtualRegisterMismatch {
        function: usize,
        register: u32,
    },
    NoLivePoints {
        function: usize,
        register: u32,
    },
    IntervalOverflow {
        function: usize,
        register: u32,
    },
    NoCommonCandidate {
        function: usize,
        register: u32,
    },
    UnknownOrIncompatibleView {
        function: usize,
        register: u32,
        view: u16,
    },
    PrefixMismatch {
        function: usize,
    },
    SecondaryPressure {
        function: usize,
        register: u32,
    },
    ReloadPressure {
        function: usize,
        result: u32,
    },
    NonCanonicalAssignment {
        function: usize,
    },
    WorkOverflow,
    UsageMismatch,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
}

impl std::fmt::Display for ReloadValueHomeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "logical reload-value home assignment failed: {self:?}"
        )
    }
}

impl std::error::Error for ReloadValueHomeError {}
