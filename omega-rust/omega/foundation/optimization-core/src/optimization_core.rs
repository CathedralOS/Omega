//! The optimization-core root: the authoritative exact-name registry of
//! optimizations and the canonical selection codec every stage and report
//! consumes. Beneath it, `post_terminal` projects the post-Terminal subset of
//! a selection, `contracts` defines rule and budget contracts, `identities`
//! owns the domain-separated identities carried between stages, `manifest`
//! their common publication records, `decisions` the baseline logs and the
//! external decision wire schema (not candidate selection),
//! `rollback` the subtractive exact-rule overlay and its receipt,
//! `report_request` the auxiliary human projection request,
//! `refusal_site` the withheld compiler location of a stage's internal check,
//! and `mutation_matrix` the test-support substitution matrix custody families
//! reuse.

pub(crate) mod contracts;
pub(crate) mod decisions;
pub(crate) mod identities;
pub(crate) mod manifest;
#[cfg(any(test, feature = "test-support"))]
pub(crate) mod mutation_matrix;
pub(crate) mod post_terminal;
pub(crate) mod refusal_site;
pub(crate) mod report_request;
pub(crate) mod rollback;

use std::fmt;

/// Which width frames one counted row's length. A wire record frames its counts
/// as u32; a canonical identity row frames them as u64. The row content is the
/// same bytes either way, so a record family writes its row once and passes the
/// frame, instead of keeping a second encoder beside the first that must agree
/// on every field and field order.
#[derive(Debug, Clone, Copy)]
pub(crate) enum CountFrame {
    Wire,
    Canonical,
}

impl CountFrame {
    pub(crate) fn push(self, encoded: &mut Vec<u8>, count: usize) {
        match self {
            Self::Wire => crate::codec_bytes::push_count_u32(encoded, count),
            Self::Canonical => crate::codec_bytes::push_count_u64(encoded, count),
        }
    }
}

/// The five-u64 work-counter row budget and usage encodings share:
/// little-endian words in declaration order across a 40-byte wire row.
pub(crate) fn encode_work_counters(fields: [u64; 5]) -> [u8; 40] {
    let mut encoded = [0; 40];
    for (index, value) in fields.into_iter().enumerate() {
        crate::codec_bytes::patch_encoded(&mut encoded, index * 8, value.to_le_bytes());
    }
    encoded
}

/// Read the exact 40-byte five-counter row back in declaration order.
pub(crate) fn decode_work_counters(encoded: &[u8]) -> Option<[u64; 5]> {
    let row: [u8; 40] = encoded.try_into().ok()?;
    let mut fields = [0; 5];
    let (chunks, _) = row.as_chunks::<8>();
    for (field, chunk) in fields.iter_mut().zip(chunks) {
        *field = u64::from_le_bytes(*chunk);
    }
    Some(fields)
}

pub use post_terminal::{
    PostTerminalOptimizationSelectionProjection, PostTerminalOptimizationSelections,
};

const SELECTION_ENCODING_MAGIC: &[u8; 8] = b"OMGOPT\0\0";
const SELECTION_ENCODING_VERSION: u32 = 15;
const SELECTION_IDENTITY_DOMAIN: &[u8] = b"omega.optimization-selections.v15\0";

crate::tagged_vocabulary! {
    9;
    /// Closed execution phase for one explicitly named optimization. Phase
    /// projection routes a complete source-visible suite; it never replaces that
    /// suite's identity or creates an optimization-level alias.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum OptimizationExecutionPhase {
        CheckedTrees = 1,
        Psi = 2,
        AbstractOperations = 3,
        TargetOperations = 4,
        SelectedLowering = 5,
        PreAllocation = 6,
        AllocationRecovery = 7,
        PostAllocationMachine = 8,
        FunctionRelativeLayout = 9,
    }
}

impl OptimizationExecutionPhase {
    /// Sealed Terminal Psi divides the phases a sealed artifact still admits
    /// from the ones that already ran: `CheckedTrees` and `Psi` execute before
    /// publication and are execution history afterwards. Every post-Terminal
    /// projection reads this split rather than restating its own phase list.
    pub const fn is_post_terminal(self) -> bool {
        !matches!(self, Self::CheckedTrees | Self::Psi)
    }
}

/// One row's value in the unified-to-Psi projection: a row that executes in
/// the `Psi` phase is source-visible to Psi under the same name, and every
/// other row stays Omega-owned.
///
/// The declared phase is the whole predicate — there is no second visibility
/// column to keep in step with the rows, and a `Psi`-phase row whose Psi
/// spelling diverged fails to compile at the projection instead of silently
/// mapping to nothing.
macro_rules! psi_projection_value {
    ($variant:ident, Psi) => {
        Some(optimization::PsiOptimization::$variant)
    };
    ($variant:ident, $phase:ident) => {
        None
    };
}

macro_rules! optimization_vocabulary {
    (
        $count:literal;
        $(
            $variant:ident = $tag:literal => {
                case: $case:literal,
                phase: $phase:ident
            }
        ),+ $(,)?
    ) => {
        /// One source-visible, semantics-preserving optimization family.
        ///
        /// Descriptor order is the canonical selection order, not an implicit
        /// optimization level or a promise about pass scheduling.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(u8)]
        pub enum Optimization {
            $($variant = $tag),+
        }

        impl Optimization {
            pub const ALL: [Self; $count] = [$(Self::$variant),+];

            pub const fn build_case_name(self) -> &'static str {
                match self { $(Self::$variant => $case),+ }
            }

            /// Resolve one exact source-visible case name. Release tooling
            /// shares this generated vocabulary rather than maintaining a
            /// second rollback-name registry.
            pub fn from_build_case_name(name: &str) -> Option<Self> {
                Self::ALL
                    .into_iter()
                    .find(|optimization| optimization.build_case_name() == name)
            }

            pub const fn execution_phase(self) -> OptimizationExecutionPhase {
                match self {
                    $(Self::$variant => OptimizationExecutionPhase::$phase),+
                }
            }

            /// Project one unified build-visible selection into Psi's
            /// independently owned pre-Terminal vocabulary. Physical
            /// optimizations have no Psi identity and remain in their
            /// Omega-owned phases.
            pub const fn optimization(self) -> Option<optimization::PsiOptimization> {
                match self {
                    $( Self::$variant => psi_projection_value!($variant, $phase) ),+
                }
            }

            fn from_tag(tag: u8) -> Result<Self, SelectionDecodeError> {
                Self::ALL
                    .into_iter()
                    .find(|optimization| *optimization as u8 == tag)
                    .ok_or(SelectionDecodeError::UnknownOptimizationTag(tag))
            }
        }
    };
}

// This is the only declaration of exact names, stable tags,
// phases, and canonical order. Build preludes are exhaustively checked against
// the generated `ALL` and `build_case_name` views.
optimization_vocabulary! {
    55;
    ControlFlowCleanup = 1 => {
        case: "ControlFlowCleanup",
        phase: Psi
    },
    SparseConditionalConstantPropagation = 2 => {
        case: "SparseConditionalConstantPropagation",
        phase: Psi
    },
    CopyPropagation = 3 => {
        case: "CopyPropagation",
        phase: Psi
    },
    GlobalValueNumbering = 4 => {
        case: "GlobalValueNumbering",
        phase: Psi
    },
    DeadPureScalarElimination = 5 => {
        case: "DeadPureScalarElimination",
        phase: Psi
    },
    ProofCheckElision = 6 => {
        case: "ProofCheckElision",
        phase: Psi
    },
    SelectedIncomingU12ExactAddImmediate = 7 => {
        case: "SelectedIncomingU12ExactAddImmediate",
        phase: SelectedLowering
    },
    X86RelaxConditionalBranchesToRel8V1 = 8 => {
        case: "X86RelaxConditionalBranchesToRel8V1",
        phase: FunctionRelativeLayout
    },
    SelectedIncomingU12ExactSubtractImmediate = 9 => {
        case: "SelectedIncomingU12ExactSubtractImmediate",
        phase: SelectedLowering
    },
    Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 = 10 => {
        case: "Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1",
        phase: PostAllocationMachine
    },
    SharedEntryFixedViewCopyAfterCompareBeforeBranchV1 = 11 => {
        case: "SharedEntryFixedViewCopyAfterCompareBeforeBranchV1",
        phase: AllocationRecovery
    },
    ActiveResidentImmediateU64MultiUseRematerializationV1 = 12 => {
        case: "ActiveResidentImmediateU64MultiUseRematerializationV1",
        phase: AllocationRecovery
    },
    Aarch64SelectShortestMovnSeededI64MaterializationV1 = 13 => {
        case: "Aarch64SelectShortestMovnSeededI64MaterializationV1",
        phase: PostAllocationMachine
    },
    X86SelectXorZeroI64MaterializationV1 = 14 => {
        case: "X86SelectXorZeroI64MaterializationV1",
        phase: PostAllocationMachine
    },
    X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1 = 15 => {
        case: "X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1",
        phase: PostAllocationMachine
    },
    X86SelectMovR64Imm32SignExtendedI64MaterializationV1 = 16 => {
        case: "X86SelectMovR64Imm32SignExtendedI64MaterializationV1",
        phase: PostAllocationMachine
    },
    Aarch64ElideSameViewCopyI64BeforeReturnV1 = 17 => {
        case: "Aarch64ElideSameViewCopyI64BeforeReturnV1",
        phase: PostAllocationMachine
    },
    Aarch64ElideSameViewCopyI64BeforeCompareZeroV1 = 18 => {
        case: "Aarch64ElideSameViewCopyI64BeforeCompareZeroV1",
        phase: PostAllocationMachine
    },
    Aarch64ElideSameViewCopyI64BeforeCompareI64LeftOperandV1 = 19 => {
        case: "Aarch64ElideSameViewCopyI64BeforeCompareI64LeftOperandV1",
        phase: PostAllocationMachine
    },
    Aarch64ElideSameViewCopyI64BeforeCompareI64RightOperandV1 = 20 => {
        case: "Aarch64ElideSameViewCopyI64BeforeCompareI64RightOperandV1",
        phase: PostAllocationMachine
    },
    SelectedIncomingU12CompareImmediate = 21 => {
        case: "SelectedIncomingU12CompareImmediate",
        phase: SelectedLowering
    },
    CheckedTreeProductPruning = 22 => {
        case: "CheckedTreeProductPruning",
        phase: CheckedTrees
    },
    SelectedIncomingLiteralExtensionElimination = 23 => {
        case: "SelectedIncomingLiteralExtensionElimination",
        phase: SelectedLowering
    },
    SelectedIncomingU12Load8IndexedOffset = 24 => {
        case: "SelectedIncomingU12Load8IndexedOffset",
        phase: SelectedLowering
    },
    SelectedIncomingLiteralCopyMaterialization = 25 => {
        case: "SelectedIncomingLiteralCopyMaterialization",
        phase: SelectedLowering
    },
    SelectedIncomingU12ByteViewAddressOffset = 26 => {
        case: "SelectedIncomingU12ByteViewAddressOffset",
        phase: SelectedLowering
    },
    SelectedIncomingExactDivideIdentityCopy = 27 => {
        case: "SelectedIncomingExactDivideIdentityCopy",
        phase: SelectedLowering
    },
    SelectedIncomingWrappingRemainderOneZeroMaterialization = 28 => {
        case: "SelectedIncomingWrappingRemainderOneZeroMaterialization",
        phase: SelectedLowering
    },
    SelectedIncomingBitwiseAndZeroMaterialization = 29 => {
        case: "SelectedIncomingBitwiseAndZeroMaterialization",
        phase: SelectedLowering
    },
    SelectedIncomingBitwiseXorZeroIdentityCopy = 30 => {
        case: "SelectedIncomingBitwiseXorZeroIdentityCopy",
        phase: SelectedLowering
    },
    SelectedIncomingWrappingAddZeroIdentityCopy = 31 => {
        case: "SelectedIncomingWrappingAddZeroIdentityCopy",
        phase: SelectedLowering
    },
    SelectedIncomingBitwiseAndOnesIdentityCopy = 32 => {
        case: "SelectedIncomingBitwiseAndOnesIdentityCopy",
        phase: SelectedLowering
    },
    SelectedIncomingWrappingRemainderZeroDividendZeroMaterialization = 33 => {
        case: "SelectedIncomingWrappingRemainderZeroDividendZeroMaterialization",
        phase: SelectedLowering
    },
    SelectedIncomingExactDivideZeroDividendZeroMaterialization = 34 => {
        case: "SelectedIncomingExactDivideZeroDividendZeroMaterialization",
        phase: SelectedLowering
    },
    SelectedIncomingSaturatingAddZeroIdentityCopy = 35 => {
        case: "SelectedIncomingSaturatingAddZeroIdentityCopy",
        phase: SelectedLowering
    },
    SelectedIncomingSaturatingSubtractZeroIdentityCopy = 36 => {
        case: "SelectedIncomingSaturatingSubtractZeroIdentityCopy",
        phase: SelectedLowering
    },
    SelectedIncomingSaturatingDivideOneIdentityCopy = 37 => {
        case: "SelectedIncomingSaturatingDivideOneIdentityCopy",
        phase: SelectedLowering
    },
    SelectedIncomingSaturatingDivideZeroDividendZeroMaterialization = 38 => {
        case: "SelectedIncomingSaturatingDivideZeroDividendZeroMaterialization",
        phase: SelectedLowering
    },
    SelectedIncomingSaturatingSubtractZeroMinuendZeroMaterialization = 39 => {
        case: "SelectedIncomingSaturatingSubtractZeroMinuendZeroMaterialization",
        phase: SelectedLowering
    },
    SelectedIncomingSaturatingAddUpperBoundMaterialization = 40 => {
        case: "SelectedIncomingSaturatingAddUpperBoundMaterialization",
        phase: SelectedLowering
    },
    SelectedIncomingWrappingRemainderMinusOneZeroMaterialization = 41 => {
        case: "SelectedIncomingWrappingRemainderMinusOneZeroMaterialization",
        phase: SelectedLowering
    },
    SelectedIncomingSaturatingSubtractUpperBoundSubtrahendZeroMaterialization = 42 => {
        case: "SelectedIncomingSaturatingSubtractUpperBoundSubtrahendZeroMaterialization",
        phase: SelectedLowering
    },
    StateSpecialization = 43 => {
        case: "StateSpecialization",
        phase: Psi
    },
    RepresentationSpecialization = 44 => {
        case: "RepresentationSpecialization",
        phase: Psi
    },
    SelectedSameBlockCopyI64RemovalV1 = 45 => {
        case: "SelectedSameBlockCopyI64RemovalV1",
        phase: PreAllocation
    },
    SelectedRedundantExtensionRemovalV1 = 46 => {
        case: "SelectedRedundantExtensionRemovalV1",
        phase: PreAllocation
    },
    SelectedAddressOffsetFoldV1 = 47 => {
        case: "SelectedAddressOffsetFoldV1",
        phase: PreAllocation
    },
    SelectedConstantBooleanFoldV1 = 48 => {
        case: "SelectedConstantBooleanFoldV1",
        phase: PreAllocation
    },
    SelectedConstantBranchFoldV1 = 49 => {
        case: "SelectedConstantBranchFoldV1",
        phase: PreAllocation
    },
    SelectedDeadCompareRemovalV1 = 50 => {
        case: "SelectedDeadCompareRemovalV1",
        phase: PreAllocation
    },
    SelectedRedundantCompareRemovalV1 = 51 => {
        case: "SelectedRedundantCompareRemovalV1",
        phase: PreAllocation
    },
    SelectedEquivalentCompareRemovalV1 = 52 => {
        case: "SelectedEquivalentCompareRemovalV1",
        phase: PreAllocation
    },
    LoopInvariantScalarMotion = 53 => {
        case: "LoopInvariantScalarMotion",
        phase: Psi
    },
    SelectedCopiedCallOperandFoldV1 = 54 => {
        case: "SelectedCopiedCallOperandFoldV1",
        phase: PreAllocation
    },
    SelectedStoredLoadForwardingV1 = 55 => {
        case: "SelectedStoredLoadForwardingV1",
        phase: PreAllocation
    },
}

/// The Psi-side half of the projection. It stays a stated match rather than
/// row expansion: the rows generate the unified-to-Psi direction, while this
/// direction names a foreign closed vocabulary and is checked exhaustive by
/// the compiler, so a new Psi optimization cannot be forgotten here.
impl From<optimization::PsiOptimization> for Optimization {
    fn from(optimization: optimization::PsiOptimization) -> Self {
        use optimization::PsiOptimization;

        match optimization {
            PsiOptimization::ControlFlowCleanup => Self::ControlFlowCleanup,
            PsiOptimization::SparseConditionalConstantPropagation => {
                Self::SparseConditionalConstantPropagation
            }
            PsiOptimization::CopyPropagation => Self::CopyPropagation,
            PsiOptimization::GlobalValueNumbering => Self::GlobalValueNumbering,
            PsiOptimization::DeadPureScalarElimination => Self::DeadPureScalarElimination,
            PsiOptimization::ProofCheckElision => Self::ProofCheckElision,
            PsiOptimization::StateSpecialization => Self::StateSpecialization,
            PsiOptimization::RepresentationSpecialization => Self::RepresentationSpecialization,
            PsiOptimization::LoopInvariantScalarMotion => Self::LoopInvariantScalarMotion,
        }
    }
}

/// Common source-visible catalog header with a representation-specific
/// payload. Target predicates, candidate constructors, validators, and route
/// types remain owned by the stage that understands them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizationCatalogDescriptor<Payload> {
    optimization: Optimization,
    payload: Payload,
}

impl<Payload> OptimizationCatalogDescriptor<Payload> {
    pub const fn new(optimization: Optimization, payload: Payload) -> Self {
        Self {
            optimization,
            payload,
        }
    }

    pub const fn optimization(&self) -> Optimization {
        self.optimization
    }

    pub const fn payload(&self) -> &Payload {
        &self.payload
    }
}

/// A canonical, duplicate-free set of explicitly selected optimizations.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OptimizationSelections {
    selected: Vec<Optimization>,
}

/// Exact phase-local projection of one complete authored selection.
///
/// The projection may be empty. It retains the complete selection identity so
/// a stage cannot present its local subset as though it were the complete
/// build-owned policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizationPhaseSelections {
    phase: OptimizationExecutionPhase,
    complete_selection: OptimizationSelectionIdentity,
    selected: OptimizationSelections,
}

/// A phase-local selection was presented to the wrong executable stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizationPhaseMismatch {
    pub expected: OptimizationExecutionPhase,
    pub actual: OptimizationExecutionPhase,
}

/// Omega-owned bridge from the complete build selection to the independently
/// encoded target-neutral Psi selection. Coordinators retain the complete
/// identity; Psi consumes only `selected`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiOptimizationSelectionProjection {
    complete_selection: OptimizationSelectionIdentity,
    selected: optimization::PsiOptimizationSelections,
}

impl PsiOptimizationSelectionProjection {
    pub const fn complete_selection(&self) -> OptimizationSelectionIdentity {
        self.complete_selection
    }

    pub const fn selections(&self) -> &optimization::PsiOptimizationSelections {
        &self.selected
    }
}

impl OptimizationPhaseSelections {
    pub const fn phase(&self) -> OptimizationExecutionPhase {
        self.phase
    }

    pub const fn complete_selection(&self) -> OptimizationSelectionIdentity {
        self.complete_selection
    }

    pub const fn selections(&self) -> &OptimizationSelections {
        &self.selected
    }

    /// Expose the local set only to the stage that owns this exact phase.
    pub fn require_phase(
        &self,
        expected: OptimizationExecutionPhase,
    ) -> Result<&OptimizationSelections, OptimizationPhaseMismatch> {
        if self.phase != expected {
            return Err(OptimizationPhaseMismatch {
                expected,
                actual: self.phase,
            });
        }
        Ok(&self.selected)
    }

    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }
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

    /// Bind one phase-local selection to the identity of the complete authored
    /// set. This is the preferred stage input; `for_phase` remains the
    /// compatibility projection for existing consumers.
    pub fn project_phase(&self, phase: OptimizationExecutionPhase) -> OptimizationPhaseSelections {
        OptimizationPhaseSelections {
            phase,
            complete_selection: self.identity(),
            selected: self.for_phase(phase),
        }
    }

    /// Exhaustively project the unified build selection into Psi's closed,
    /// target-neutral pre-Terminal vocabulary.
    pub fn project_psi(&self) -> PsiOptimizationSelectionProjection {
        let selected = optimization::PsiOptimizationSelections::new(
            self.selected
                .iter()
                .filter_map(|optimization| optimization.optimization()),
        )
        .expect("the unified-to-Psi optimization map is injective");
        PsiOptimizationSelectionProjection {
            complete_selection: self.identity(),
            selected,
        }
    }

    /// Project the exact selected suite onto phases owned by the consumer of a
    /// sealed Terminal artifact. Psi selections are execution history in that
    /// artifact and may not be selected again by its lowerer.
    pub fn project_post_terminal(&self) -> PostTerminalOptimizationSelectionProjection {
        PostTerminalOptimizationSelectionProjection {
            complete_selection: self.identity(),
            complete_selections: self.clone(),
            selected: PostTerminalOptimizationSelections {
                selected: Self {
                    selected: self
                        .selected
                        .iter()
                        .copied()
                        .filter(|optimization| optimization.execution_phase().is_post_terminal())
                        .collect(),
                },
            },
        }
    }

    /// Canonical standalone encoding for cache, artifact, and replay inputs.
    pub fn encode(&self) -> Vec<u8> {
        let mut encoded = Vec::with_capacity(16 + self.selected.len());
        encoded.extend_from_slice(SELECTION_ENCODING_MAGIC);
        crate::codec_bytes::push_encoded(&mut encoded, SELECTION_ENCODING_VERSION.to_le_bytes());
        crate::codec_bytes::push_count_u32(&mut encoded, self.selected.len());
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
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(SELECTION_IDENTITY_DOMAIN);
        digest.bytes(&encoded);
        OptimizationSelectionIdentity(digest.finish())
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
mod tests;

/// Lowercase hex rendering for byte payloads in manifests and identities:
/// the digest module's renderer is the crate's one hex implementation.
pub use crate::digest_encoder::hex;

/// Tagged optional u64 payload: 0 discriminant, or 1 followed by 8 LE bytes.
pub fn encode_option_u64(bytes: &mut Vec<u8>, value: Option<u64>) {
    match value {
        Some(value) => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
        }
        None => bytes.push(0),
    }
}
