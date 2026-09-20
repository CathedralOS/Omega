//! Durable fixed-view-copy plan and canonical artifact codec.
//!
//! The plan is retained evidence: decoding it grants no validation authority.

pub(crate) mod codec;
mod identity;
pub use identity::fixed_view_copy_identity;

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use optimization_unit::ValueDefinitionSite;
use register_model::{RegisterConstraintKey, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::{
    LiveRangeIdentity, SelectedBlockId, SelectedInstructionId, SelectedInstructionPlan,
    SelectedInstructionPlanIdentity, VirtualFixedConstraintSite, VirtualRegisterId,
};
use semantic_vocabulary::{MachineId, ValueId};

use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FixedPrecoloredIntervalPlanIdentity,
    FixedPrecoloredSegmentHomePlanIdentity, FixedPrecoloredSplitRequirementPlanIdentity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixedViewCopyPolicy {
    /// One copy immediately before each fixed leaf use.
    LeafLocalBeforeFixedUseV1,
    /// The declared recovery selection: the shared-exit leg under its
    /// entry-parameter admission gate — one copy at each shared source
    /// segment end — refusing any boundary the partition cannot share.
    SharedEntryAfterCompareBeforeBranchV1,
    /// One copy in the fixed-use site's own block immediately before the
    /// instruction, for any operand `Use` site of a source-scalar `u64`
    /// register. The site may sit mid-block or on a terminator, entry block
    /// or leaf, and the boundary's source may be any pinned view of the
    /// register, not only its live-in.
    ImmediateBeforeFixedUseV1,
    /// Boundaries leaving one source segment end through connectors out of a
    /// single block's terminator share one copy inserted there — the point
    /// dominating every member site — while any other boundary keeps a copy
    /// at its own fixed-use site. Origin admission matches the immediate
    /// form.
    SharedSourceExitBeforeFixedUseV1,
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
    UnknownCrashCause(u8),
    InvalidCrashPredicate,
    UnknownValueTransport(u8),
    UnknownBlockOrigin(u8),
    UnknownSuccessorRole(u8),
    UnknownInstructionKind(u8),
    InvalidPackedByteWidth(u8),
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
    InvalidForeignTargetProfile,
    InvalidForeignLocator,
    UnknownForeignLocatorCase(u8),
    UnknownMachineRegime(u8),
    UnknownEntryStack(u8),
    UnknownPreemption(u8),
    InvalidMachineStateSet(u16),
    UnknownForeignScalarSource(u8),
    InvalidSameStackContribution,
    InvalidRankedCustody,
    InvalidCrashContinuations,
    InvalidStructuralTransport,
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
