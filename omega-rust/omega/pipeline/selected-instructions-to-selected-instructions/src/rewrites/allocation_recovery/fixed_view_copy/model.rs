//! Transform-side evidence for the fixed-view-copy rewrite; the durable plan,
//! policy and artifact framing live in `register_homes::recovery`.

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions::{
    FixedViewCopyIdentity, LiveRangeIdentity, SelectedInstructionPlanIdentity,
};

use crate::{FixedViewCopyPlan, FixedViewCopyPolicy, FixedViewCopySourceEvidence};
use register_homes::{AllocationLegalityIdentity, AllocatorAvailabilityIdentity};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedViewCopyValidationReceipt {
    pub(crate) identity: FixedViewCopyIdentity,
    pub(crate) source_selected: SelectedInstructionPlanIdentity,
    pub(crate) source_ranges: LiveRangeIdentity,
    pub(crate) source_legality: AllocationLegalityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) source_evidence: FixedViewCopySourceEvidence,
    pub(crate) transformed_selected: SelectedInstructionPlanIdentity,
    pub(crate) optimization_unit: optimization_core::OptimizationUnitIdentity,
    pub(crate) fuel_schedule: semantic_vocabulary::FuelScheduleIdentity,
    pub(crate) policy: FixedViewCopyPolicy,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) copy_count: usize,
}

impl FixedViewCopyValidationReceipt {
    pub const fn identity(self) -> FixedViewCopyIdentity {
        self.identity
    }
    pub const fn source_selected(self) -> SelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn source_ranges(self) -> LiveRangeIdentity {
        self.source_ranges
    }
    pub const fn source_legality(self) -> AllocationLegalityIdentity {
        self.source_legality
    }
    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn allocator_availability(self) -> AllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    pub const fn source_evidence(self) -> FixedViewCopySourceEvidence {
        self.source_evidence
    }
    pub const fn transformed_selected(self) -> SelectedInstructionPlanIdentity {
        self.transformed_selected
    }
    pub const fn optimization_unit(self) -> optimization_core::OptimizationUnitIdentity {
        self.optimization_unit
    }
    pub const fn fuel_schedule(self) -> semantic_vocabulary::FuelScheduleIdentity {
        self.fuel_schedule
    }
    pub const fn policy(self) -> FixedViewCopyPolicy {
        self.policy
    }
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn copy_count(self) -> usize {
        self.copy_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFixedViewCopies {
    pub(crate) plan: FixedViewCopyPlan,
    pub(crate) receipt: FixedViewCopyValidationReceipt,
}

impl ValidatedFixedViewCopies {
    pub const fn plan(&self) -> &FixedViewCopyPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> FixedViewCopyValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedViewCopyError {
    RootMismatch,
    WorkOverflow,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
    FunctionMismatch {
        function: usize,
    },
    UnsupportedPolicy,
    LegacySourceEvidence,
    SegmentEvidenceMismatch,
    UnsupportedSegmentBoundarySet {
        function: usize,
    },
    UnsupportedTransitionSite {
        function: usize,
        register: u32,
    },
    UnsupportedSourceRegister {
        function: usize,
        register: u32,
    },
    MissingDestination {
        function: usize,
        instruction: u32,
    },
    NonLeafDestination {
        function: usize,
        instruction: u32,
    },
    UnsupportedSharedTransitionSet {
        function: usize,
    },
    InvalidInsertionSite {
        function: usize,
        instruction: u32,
    },
    CopyConstraintMismatch,
    IdentifierOverflow {
        function: usize,
    },
    NonCanonicalCopies,
    CopyMismatch {
        index: usize,
    },
    TransformedPlanMismatch,
    ReceiptMismatch,
}

impl std::fmt::Display for FixedViewCopyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Terminal fixed-view copy materialization failed: {self:?}"
        )
    }
}

impl std::error::Error for FixedViewCopyError {}
