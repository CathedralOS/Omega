use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, LiveRangeIdentity,
    RegisterHomeIdentity, RegisterHomePlan,
};
use omega_register_model::TargetRegisterEnvironmentIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterHomeValidationReceipt {
    pub(crate) identity: RegisterHomeIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) function_count: usize,
    pub(crate) structural_unit_function_count: usize,
    pub(crate) assignment_count: usize,
    pub(crate) tied_pair_count: usize,
    pub(crate) tied_component_count: usize,
    pub(crate) early_clobber_count: usize,
}

impl RegisterHomeValidationReceipt {
    pub const fn identity(self) -> RegisterHomeIdentity {
        self.identity
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
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn structural_unit_function_count(self) -> usize {
        self.structural_unit_function_count
    }
    pub const fn assignment_count(self) -> usize {
        self.assignment_count
    }
    pub const fn tied_pair_count(self) -> usize {
        self.tied_pair_count
    }
    pub const fn tied_component_count(self) -> usize {
        self.tied_component_count
    }
    pub const fn early_clobber_count(self) -> usize {
        self.early_clobber_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRegisterHomes {
    pub(crate) plan: std::sync::Arc<RegisterHomePlan>,
    pub(crate) receipt: RegisterHomeValidationReceipt,
}

impl ValidatedRegisterHomes {
    /// Share current immutable data; the returned artifact grants no new authority.
    pub fn shared_plan(&self) -> std::sync::Arc<RegisterHomePlan> {
        std::sync::Arc::clone(&self.plan)
    }

    pub fn plan(&self) -> &RegisterHomePlan {
        &self.plan
    }
    pub const fn receipt(&self) -> RegisterHomeValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterHomeError {
    RootMismatch,
    FunctionMismatch {
        function: usize,
    },
    VirtualRegisterMismatch {
        function: usize,
        register: u32,
    },
    UnresolvedEntryTransitions {
        function: usize,
        register: u32,
        count: usize,
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
    NoCompatibleHome {
        function: usize,
        register: u32,
    },
    UnsupportedTiedTopology {
        function: usize,
        instruction: u32,
    },
    TiedRegistersInterfere {
        function: usize,
        lower: u32,
        higher: u32,
    },
    NoCommonTiedComponent {
        function: usize,
        leader: u32,
        member_count: usize,
    },
    NonCanonicalAssignments {
        function: usize,
    },
}

impl std::fmt::Display for RegisterHomeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Terminal register-home assignment failed: {self:?}"
        )
    }
}

impl std::error::Error for RegisterHomeError {}
