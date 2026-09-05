use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_optimization_unit::ValueDefinitionSite;
use omega_register_model::{
    RegisterConstraintKey, RegisterViewId, TargetRegisterEnvironmentIdentity,
};
use omega_selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlan,
    SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use psi_core::{MachineId, ValueId};

use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FixedPrecoloredIntervalPlanIdentity,
    FixedPrecoloredSegmentHomePlanIdentity, FixedPrecoloredSplitRequirementPlanIdentity,
    LiveRangeIdentity, VirtualFixedConstraintSite,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FixedViewCopyIdentity(pub(crate) [u8; 32]);

impl FixedViewCopyIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Exact, deliberately narrow policy for materializing entry-to-fixed-use
/// transitions. This is a stable named transformation, not an allocator mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixedViewCopyPolicy {
    /// One copy immediately before each fixed leaf use.
    LeafLocalBeforeFixedUseV1,
    /// One flag-transparent copy after the entry compare and immediately
    /// before its conditional branch, shared by both return leaves.
    SharedEntryAfterCompareBeforeBranchV1,
}

/// Authenticated authority used to discover the exact fixed-view boundaries
/// consumed by this transformation. Legacy wire generations remain decodable,
/// but current production and validation require segment-home evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixedViewCopySourceEvidence {
    LegacyLegalityTransitionsV1,
    FixedPrecoloredSegmentHomesV1 {
        fixed_intervals: FixedPrecoloredIntervalPlanIdentity,
        split_requirements: FixedPrecoloredSplitRequirementPlanIdentity,
        segment_homes: FixedPrecoloredSegmentHomePlanIdentity,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedViewCopyPlan {
    pub source_selected: SelectedInstructionPlanIdentity,
    pub source_ranges: LiveRangeIdentity,
    pub source_legality: AllocationLegalityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub source_evidence: FixedViewCopySourceEvidence,
    pub policy: FixedViewCopyPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub copies: Vec<FixedViewCopy>,
    pub transformed: std::sync::Arc<SelectedInstructionPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedViewCopy {
    pub function: u32,
    pub machine: MachineId,
    pub source_virtual_register: VirtualRegisterId,
    pub source_value: ValueId,
    pub source_definition_site: ValueDefinitionSite,
    pub from_view: RegisterViewId,
    /// Common destination view. Every destination row must repeat this view.
    pub to_view: RegisterViewId,
    pub insertion_block: SelectedBlockId,
    pub before_instruction: SelectedInstructionId,
    pub destinations: Vec<FixedViewCopyDestination>,
    pub copy_instruction: SelectedInstructionId,
    pub result_virtual_register: VirtualRegisterId,
    pub copy_constraint: RegisterConstraintKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedViewCopyDestination {
    /// Exact fixed-use site rewritten to the copy result.
    pub site: VirtualFixedConstraintSite,
    /// Leaf containing `site`.
    pub block: SelectedBlockId,
    /// Fixed view required by `site`; equal to the action-wide `to_view`.
    pub view: RegisterViewId,
}

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
    pub(crate) optimization_unit: omega_optimization_core::OptimizationUnitIdentity,
    pub(crate) fuel_schedule: psi_core::FuelScheduleIdentity,
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
    pub const fn optimization_unit(self) -> omega_optimization_core::OptimizationUnitIdentity {
        self.optimization_unit
    }
    pub const fn fuel_schedule(self) -> psi_core::FuelScheduleIdentity {
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

/// Artifact framing errors. Successful decoding returns an unchecked plain
/// plan and does not replace independent fixed-view-copy validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedViewCopyDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownPolicy(u8),
    UnknownSourceEvidence(u8),
    UnknownDefinitionSite(u8),
    UnknownFixedSite(u8),
    UnknownRegisterOrigin(u8),
    UnknownTerminator(u8),
    UnknownInstructionKind(u8),
    UnknownFuelSite(u8),
    UnknownArchitecture(u8),
    UnknownObjectFormat(u8),
    UnknownScalarType(u8),
    UnknownIntegerCarrier(u8),
    UnknownIntegerSign(u8),
    UnknownIntegerValue(u8),
    UnknownConstraintFamily(u8),
    UnknownOperandAccess(u8),
    UnknownBoolean(u8),
    UnknownOption(u8),
    UnknownStructuralAbiRecipe(u8),
    UnknownMachineRegister(u8),
    InvalidMachineRegisterPayload(u8),
    UnknownCallingPolicy(u8),
    UnknownEntryControl(u8),
    UnknownNativePlace(u8),
    UnknownValueClass(u8),
    UnknownSystemVEightbyteClass(u8),
    UnknownValueLocation(u8),
    UnknownIndirectPointer(u8),
    UnknownStructuralMultiplicity(u8),
    UnknownStructuralAccess(u8),
    UnknownStructuralTypeShape(u8),
    UnknownByteSequenceCarrier(u8),
    UnknownBindingRelevance(u8),
    UnknownStructuralFieldType(u8),
    UnknownIeeeFloatFormat(u8),
    UnknownStructuralPlaceKind(u8),
    UnknownStructuralPathSegment(u8),
    UnknownCallSource(u8),
    UnknownBoundaryRealization(u8),
    UnknownContentPlaceVersion(u8),
    UnknownContentPlaceSegment(u8),
    UnknownContentAlgebra(u8),
    UnknownOwnershipEvent(u8),
    UnknownCleanupAction(u8),
    InvalidVocabulary(u16),
    InvalidFuelSchedule(u32),
    InvalidSemanticId(u64),
    InvalidIntegerType,
    InvalidBudget,
    InvalidUsage,
    InvalidUtf8,
    InvalidNominalId(u64),
    InvalidProviderExecution,
    InvalidCrashContinuations,
    LengthOverflow,
    TransformedIdentityMismatch,
    TransformedPayloadMismatch,
    IdentityMismatch,
    TrailingBytes,
}

impl std::fmt::Display for FixedViewCopyDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid Terminal fixed-view-copy artifact: {self:?}"
        )
    }
}

impl std::error::Error for FixedViewCopyDecodeError {}
