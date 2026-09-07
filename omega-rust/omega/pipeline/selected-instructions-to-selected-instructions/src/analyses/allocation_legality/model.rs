use crate::*;
use register_model::TargetRegisterEnvironmentIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocationLegalityValidationReceipt {
    pub(crate) identity: AllocationLegalityIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) function_count: usize,
    pub(crate) structural_unit_function_count: usize,
    pub(crate) virtual_register_count: usize,
    pub(crate) point_count: usize,
    pub(crate) candidate_count: usize,
    pub(crate) early_clobber_point_count: usize,
    pub(crate) early_clobber_candidate_count: usize,
    pub(crate) entry_transition_count: usize,
}

impl AllocationLegalityValidationReceipt {
    pub const fn identity(self) -> AllocationLegalityIdentity {
        self.identity
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
    pub const fn virtual_register_count(self) -> usize {
        self.virtual_register_count
    }
    pub const fn point_count(self) -> usize {
        self.point_count
    }
    pub const fn candidate_count(self) -> usize {
        self.candidate_count
    }
    pub const fn early_clobber_point_count(self) -> usize {
        self.early_clobber_point_count
    }
    pub const fn early_clobber_candidate_count(self) -> usize {
        self.early_clobber_candidate_count
    }
    pub const fn entry_transition_count(self) -> usize {
        self.entry_transition_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAllocationLegality {
    pub(crate) plan: std::sync::Arc<AllocationLegalityPlan>,
    pub(crate) receipt: AllocationLegalityValidationReceipt,
}

impl ValidatedAllocationLegality {
    pub fn plan(&self) -> &AllocationLegalityPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> AllocationLegalityValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocationLegalityError {
    RootMismatch,
    UnknownClass {
        function: usize,
        register: u32,
        class: u16,
    },
    UnknownFixedView {
        function: usize,
        register: u32,
        view: u16,
    },
    IllegalFixedView {
        function: usize,
        register: u32,
        view: u16,
    },
    NoCandidateViews {
        function: usize,
        register: u32,
        block: u32,
        point: u32,
    },
    PointOverflow {
        function: usize,
    },
    FunctionMismatch {
        function: usize,
    },
    VirtualRegisterMismatch {
        function: usize,
        register: u32,
    },
    NonCanonicalRows {
        function: usize,
        register: u32,
    },
}

impl std::fmt::Display for AllocationLegalityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Terminal allocation-legality derivation failed: {self:?}"
        )
    }
}

impl std::error::Error for AllocationLegalityError {}
