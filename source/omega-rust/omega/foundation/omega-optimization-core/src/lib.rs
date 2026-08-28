//! Stable, target-independent identities for Omega optimization inputs.
//!
//! This crate deliberately owns no optimizer registry, analysis manager, cost
//! model, or executable rewrite. An empty [`OptimizationSelections`] value is
//! therefore sufficient to keep the ordinary compiler path from constructing
//! optimizer machinery while explicit build selection is being brought up.

use sha2::{Digest, Sha256};
use std::fmt;

mod contracts;
mod identities;
mod manifest;

pub use contracts::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, CoreContractDecodeError,
    InvalidOptimizationRuleContract, InvalidOptimizationWorkBudget, OptimizationCandidateVerdict,
    OptimizationReasonCode, OptimizationRuleContract, OptimizationSafetyClass,
    OptimizationWorkBudget,
};
pub use identities::{
    AcceptedObligationFactIdentity, DuplicateOptimizationRuleIdentity,
    FunctionFragmentEmissionManifestIdentity, FunctionFragmentObjectContainerManifestIdentity,
    FunctionFragmentTextSectionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, IdentityBundleDecodeError,
    IdentityDecodeError, OptimizationCandidateIdentity, OptimizationDecisionIdentity,
    OptimizationDecisionLogIdentity, OptimizationDecisionSchemaIdentity,
    OptimizationDecisionTargetIdentity, OptimizationIdentityBundle,
    OptimizationIdentityBundleIdentity, OptimizationPassIdentity, OptimizationRuleIdentity,
    OptimizationRuleSetIdentity, OptimizationUnitIdentity, OptimizationValidatorIdentity,
    OptimizationWorkloadProfileIdentity, OptimizedAbstractPlanProjectionIdentity,
    OptimizedTerminalObjectArtifactIdentity, OptimizedTerminalObjectArtifactManifestIdentity,
    OptimizedTerminalOrdinaryCallableEntryIdentity,
    OptimizedTerminalOrdinaryCallableEntryManifestIdentity, OwnershipFrontierFactIdentity,
    PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
    ScalarConstantFactIdentity, SelectedLoweringOptimizationCompletionIdentity,
    TargetCostModelIdentity, TerminalFunctionFragmentEmissionIdentity,
    TerminalRelocationFreeObjectContainerIdentity, TerminalRelocationFreeObjectPlanIdentity,
    TerminalRelocationFreeTextSectionIdentity, TransformationLedgerIdentity,
};
pub use manifest::{
    InvalidOptimizationManifestRecord, OptimizationDecisionRecord, OptimizationFactReference,
    OptimizationManifestDecodeError, OptimizationPassManifestRecord, OptimizationWorkUsage,
};

const SELECTION_ENCODING_MAGIC: &[u8; 8] = b"OMGOPT\0\0";
const SELECTION_ENCODING_VERSION: u32 = 7;
const SELECTION_IDENTITY_DOMAIN: &[u8] = b"omega.optimization-selections.v7\0";

/// Closed execution phase for one explicitly named optimization. Phase
/// projection routes a complete source-visible suite; it never replaces that
/// suite's identity or creates an optimization-level alias.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OptimizationExecutionPhase {
    Psi,
    SelectedLowering,
    AllocationRecovery,
    PostAllocationMachine,
    FunctionRelativeLayout,
}

/// One source-visible, semantics-preserving optimization family.
///
/// Declaration order is the canonical order used by selection encodings. It
/// is not a promise about pass scheduling; the pass manager will derive that
/// schedule from the exact selected set and declared dependencies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Optimization {
    ControlFlowCleanup = 1,
    SparseConditionalConstantPropagation = 2,
    CopyPropagation = 3,
    GlobalValueNumbering = 4,
    DeadPureScalarElimination = 5,
    ProofCheckElision = 6,
    SelectedIncomingU12ExactAddImmediate = 7,
    X86RelaxConditionalBranchesToRel8V1 = 8,
    SelectedIncomingU12ExactSubtractImmediate = 9,
    Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 = 10,
    SharedEntryFixedViewCopyAfterCompareBeforeBranchV1 = 11,
    ActiveResidentImmediateU64MultiUseRematerializationV1 = 12,
}

impl Optimization {
    pub const ALL: [Self; 12] = [
        Self::ControlFlowCleanup,
        Self::SparseConditionalConstantPropagation,
        Self::CopyPropagation,
        Self::GlobalValueNumbering,
        Self::DeadPureScalarElimination,
        Self::ProofCheckElision,
        Self::SelectedIncomingU12ExactAddImmediate,
        Self::X86RelaxConditionalBranchesToRel8V1,
        Self::SelectedIncomingU12ExactSubtractImmediate,
        Self::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
        Self::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
        Self::ActiveResidentImmediateU64MultiUseRematerializationV1,
    ];

    pub const fn build_case_name(self) -> &'static str {
        match self {
            Self::ControlFlowCleanup => "ControlFlowCleanup",
            Self::SparseConditionalConstantPropagation => "SparseConditionalConstantPropagation",
            Self::CopyPropagation => "CopyPropagation",
            Self::GlobalValueNumbering => "GlobalValueNumbering",
            Self::DeadPureScalarElimination => "DeadPureScalarElimination",
            Self::ProofCheckElision => "ProofCheckElision",
            Self::SelectedIncomingU12ExactAddImmediate => "SelectedIncomingU12ExactAddImmediate",
            Self::X86RelaxConditionalBranchesToRel8V1 => "X86RelaxConditionalBranchesToRel8V1",
            Self::SelectedIncomingU12ExactSubtractImmediate => {
                "SelectedIncomingU12ExactSubtractImmediate"
            }
            Self::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 => {
                "Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1"
            }
            Self::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1 => {
                "SharedEntryFixedViewCopyAfterCompareBeforeBranchV1"
            }
            Self::ActiveResidentImmediateU64MultiUseRematerializationV1 => {
                "ActiveResidentImmediateU64MultiUseRematerializationV1"
            }
        }
    }

    pub const fn build_counter_field(self) -> &'static str {
        match self {
            Self::ControlFlowCleanup => "control_flow_cleanup",
            Self::SparseConditionalConstantPropagation => "sparse_conditional_constant_propagation",
            Self::CopyPropagation => "copy_propagation",
            Self::GlobalValueNumbering => "global_value_numbering",
            Self::DeadPureScalarElimination => "dead_pure_scalar_elimination",
            Self::ProofCheckElision => "proof_check_elision",
            Self::SelectedIncomingU12ExactAddImmediate => {
                "selected_incoming_u12_exact_add_immediate"
            }
            Self::X86RelaxConditionalBranchesToRel8V1 => {
                "x86_relax_conditional_branches_to_rel8_v1"
            }
            Self::SelectedIncomingU12ExactSubtractImmediate => {
                "selected_incoming_u12_exact_subtract_immediate"
            }
            Self::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 => {
                "aarch64_fuse_compare_i64_zero_branch_nonzero_to_cbnz_v1"
            }
            Self::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1 => {
                "shared_entry_fixed_view_copy_after_compare_before_branch_v1"
            }
            Self::ActiveResidentImmediateU64MultiUseRematerializationV1 => {
                "active_resident_immediate_u64_multi_use_rematerialization_v1"
            }
        }
    }

    pub const fn execution_phase(self) -> OptimizationExecutionPhase {
        match self {
            Self::ControlFlowCleanup
            | Self::SparseConditionalConstantPropagation
            | Self::CopyPropagation
            | Self::GlobalValueNumbering
            | Self::DeadPureScalarElimination
            | Self::ProofCheckElision => OptimizationExecutionPhase::Psi,
            Self::SelectedIncomingU12ExactAddImmediate
            | Self::SelectedIncomingU12ExactSubtractImmediate => {
                OptimizationExecutionPhase::SelectedLowering
            }
            Self::X86RelaxConditionalBranchesToRel8V1 => {
                OptimizationExecutionPhase::FunctionRelativeLayout
            }
            Self::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 => {
                OptimizationExecutionPhase::PostAllocationMachine
            }
            Self::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1
            | Self::ActiveResidentImmediateU64MultiUseRematerializationV1 => {
                OptimizationExecutionPhase::AllocationRecovery
            }
        }
    }

    fn from_tag(tag: u8) -> Result<Self, SelectionDecodeError> {
        Self::ALL
            .into_iter()
            .find(|optimization| *optimization as u8 == tag)
            .ok_or(SelectionDecodeError::UnknownOptimizationTag(tag))
    }
}

/// A canonical, duplicate-free set of explicitly selected optimizations.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OptimizationSelections {
    selected: Vec<Optimization>,
}

impl OptimizationSelections {
    pub fn new(
        selected: impl IntoIterator<Item = Optimization>,
    ) -> Result<Self, DuplicateOptimization> {
        let mut selected = selected.into_iter().collect::<Vec<_>>();
        selected.sort_unstable();
        for pair in selected.windows(2) {
            if pair[0] == pair[1] {
                return Err(DuplicateOptimization(pair[0]));
            }
        }
        Ok(Self { selected })
    }

    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    pub fn as_slice(&self) -> &[Optimization] {
        &self.selected
    }

    pub fn contains(&self, optimization: Optimization) -> bool {
        self.selected.binary_search(&optimization).is_ok()
    }

    /// Canonical subset routed to one execution phase. The complete selection
    /// remains the identity-bearing optimizer input.
    pub fn for_phase(&self, phase: OptimizationExecutionPhase) -> Self {
        Self {
            selected: self
                .selected
                .iter()
                .copied()
                .filter(|optimization| optimization.execution_phase() == phase)
                .collect(),
        }
    }

    /// Canonical standalone encoding for cache, artifact, and replay inputs.
    pub fn encode(&self) -> Vec<u8> {
        let count = u32::try_from(self.selected.len())
            .expect("the closed optimization vocabulary fits in u32");
        let mut encoded = Vec::with_capacity(16 + self.selected.len());
        encoded.extend_from_slice(SELECTION_ENCODING_MAGIC);
        encoded.extend_from_slice(&SELECTION_ENCODING_VERSION.to_le_bytes());
        encoded.extend_from_slice(&count.to_le_bytes());
        encoded.extend(self.selected.iter().map(|optimization| *optimization as u8));
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, SelectionDecodeError> {
        if encoded.len() < 16 {
            return Err(SelectionDecodeError::Truncated);
        }
        if &encoded[..8] != SELECTION_ENCODING_MAGIC {
            return Err(SelectionDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(encoded[8..12].try_into().expect("fixed version width"));
        if version != SELECTION_ENCODING_VERSION {
            return Err(SelectionDecodeError::UnsupportedVersion(version));
        }
        let count = u32::from_le_bytes(encoded[12..16].try_into().expect("fixed count width"));
        let count = usize::try_from(count).map_err(|_| SelectionDecodeError::Truncated)?;
        let expected = 16usize
            .checked_add(count)
            .ok_or(SelectionDecodeError::Truncated)?;
        if encoded.len() < expected {
            return Err(SelectionDecodeError::Truncated);
        }
        if encoded.len() > expected {
            return Err(SelectionDecodeError::TrailingBytes);
        }
        let mut previous = None;
        let mut selected = Vec::with_capacity(count);
        for tag in &encoded[16..] {
            let optimization = Optimization::from_tag(*tag)?;
            if let Some(previous) = previous {
                if optimization == previous {
                    return Err(SelectionDecodeError::Duplicate(optimization));
                }
                if optimization < previous {
                    return Err(SelectionDecodeError::NonCanonicalOrder);
                }
            }
            previous = Some(optimization);
            selected.push(optimization);
        }
        Ok(Self { selected })
    }

    /// Domain-separated digest of the exact canonical selected set.
    pub fn identity(&self) -> OptimizationSelectionIdentity {
        let encoded = self.encode();
        let mut digest = Sha256::new();
        digest.update(SELECTION_IDENTITY_DOMAIN);
        digest.update(
            u64::try_from(encoded.len())
                .expect("selection encoding length fits u64")
                .to_le_bytes(),
        );
        digest.update(encoded);
        OptimizationSelectionIdentity(digest.finalize().into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OptimizationSelectionIdentity([u8; 32]);

impl OptimizationSelectionIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuplicateOptimization(pub Optimization);

impl fmt::Display for DuplicateOptimization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "optimization `{}` is selected more than once",
            self.0.build_case_name()
        )
    }
}

impl std::error::Error for DuplicateOptimization {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownOptimizationTag(u8),
    Duplicate(Optimization),
    NonCanonicalOrder,
    TrailingBytes,
}

impl fmt::Display for SelectionDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => formatter.write_str("optimization selection encoding is truncated"),
            Self::WrongMagic => {
                formatter.write_str("optimization selection encoding has wrong magic")
            }
            Self::UnsupportedVersion(version) => write!(
                formatter,
                "optimization selection encoding version {version} is unsupported"
            ),
            Self::UnknownOptimizationTag(tag) => {
                write!(formatter, "optimization selection has unknown tag {tag}")
            }
            Self::Duplicate(optimization) => write!(
                formatter,
                "optimization selection repeats `{}`",
                optimization.build_case_name()
            ),
            Self::NonCanonicalOrder => {
                formatter.write_str("optimization selection is not in canonical order")
            }
            Self::TrailingBytes => {
                formatter.write_str("optimization selection encoding has trailing bytes")
            }
        }
    }
}

impl std::error::Error for SelectionDecodeError {}

#[cfg(test)]
mod tests {
    use super::{
        Optimization, OptimizationExecutionPhase, OptimizationSelections, SelectionDecodeError,
    };

    #[test]
    fn selections_are_sorted_and_round_trip_canonically() {
        let selections = OptimizationSelections::new([
            Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
            Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
            Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
            Optimization::X86RelaxConditionalBranchesToRel8V1,
            Optimization::ProofCheckElision,
            Optimization::ControlFlowCleanup,
            Optimization::CopyPropagation,
        ])
        .expect("unique selections");
        assert_eq!(
            selections.as_slice(),
            &[
                Optimization::ControlFlowCleanup,
                Optimization::CopyPropagation,
                Optimization::ProofCheckElision,
                Optimization::X86RelaxConditionalBranchesToRel8V1,
                Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
                Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
                Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
            ]
        );
        let encoded = selections.encode();
        assert_eq!(
            OptimizationSelections::decode(&encoded).expect("canonical decode"),
            selections
        );
        assert_eq!(
            OptimizationSelections::decode(&encoded)
                .expect("repeat decode")
                .encode(),
            encoded
        );
    }

    #[test]
    fn duplicates_reject_before_identity() {
        let error = OptimizationSelections::new([
            Optimization::GlobalValueNumbering,
            Optimization::GlobalValueNumbering,
        ])
        .expect_err("duplicate selection must reject");
        assert_eq!(error.0, Optimization::GlobalValueNumbering);
    }

    #[test]
    fn decoder_rejects_noncanonical_and_trailing_encodings() {
        let selections = OptimizationSelections::new([
            Optimization::ControlFlowCleanup,
            Optimization::CopyPropagation,
        ])
        .expect("unique selections");
        let mut reversed = selections.encode();
        reversed[16..].reverse();
        assert_eq!(
            OptimizationSelections::decode(&reversed),
            Err(SelectionDecodeError::NonCanonicalOrder)
        );

        let mut trailing = selections.encode();
        trailing.push(0);
        assert_eq!(
            OptimizationSelections::decode(&trailing),
            Err(SelectionDecodeError::TrailingBytes)
        );

        let mut old_version = selections.encode();
        old_version[8..12].copy_from_slice(&6_u32.to_le_bytes());
        assert_eq!(
            OptimizationSelections::decode(&old_version),
            Err(SelectionDecodeError::UnsupportedVersion(6))
        );
    }

    #[test]
    fn identity_is_domain_stable_and_selection_sensitive() {
        let empty = OptimizationSelections::default().identity();
        let selected = OptimizationSelections::new([Optimization::ControlFlowCleanup])
            .expect("unique selection")
            .identity();
        assert_ne!(empty, selected);
        assert_eq!(empty, OptimizationSelections::default().identity());
    }

    #[test]
    fn phase_projection_is_canonical_without_replacing_the_full_identity() {
        let selections = OptimizationSelections::new([
            Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
            Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
            Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
            Optimization::X86RelaxConditionalBranchesToRel8V1,
            Optimization::SelectedIncomingU12ExactAddImmediate,
            Optimization::SelectedIncomingU12ExactSubtractImmediate,
            Optimization::CopyPropagation,
            Optimization::SparseConditionalConstantPropagation,
        ])
        .unwrap();
        let full_identity = selections.identity();
        assert_eq!(
            selections
                .for_phase(OptimizationExecutionPhase::AllocationRecovery)
                .as_slice(),
            &[
                Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
                Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
            ]
        );
        assert_eq!(
            selections
                .for_phase(OptimizationExecutionPhase::PostAllocationMachine)
                .as_slice(),
            &[Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1]
        );
        assert_eq!(
            selections
                .for_phase(OptimizationExecutionPhase::Psi)
                .as_slice(),
            &[
                Optimization::SparseConditionalConstantPropagation,
                Optimization::CopyPropagation,
            ]
        );
        assert_eq!(
            selections
                .for_phase(OptimizationExecutionPhase::SelectedLowering)
                .as_slice(),
            &[
                Optimization::SelectedIncomingU12ExactAddImmediate,
                Optimization::SelectedIncomingU12ExactSubtractImmediate,
            ]
        );
        assert_eq!(
            selections
                .for_phase(OptimizationExecutionPhase::FunctionRelativeLayout)
                .as_slice(),
            &[Optimization::X86RelaxConditionalBranchesToRel8V1]
        );
        assert_eq!(full_identity, selections.identity());
        assert_ne!(
            full_identity,
            selections
                .for_phase(OptimizationExecutionPhase::Psi)
                .identity()
        );
    }
}
