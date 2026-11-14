/*
Authoritative exact-name registry of source-visible optimizations, their
execution phases, and the canonical selection codec (`OMGOPT\0\0` v16).

We declare every optimization once in `optimization_vocabulary!`, binding its
stable wire tag, exact `build.omg` case name, and `OptimizationExecutionPhase`.
Descriptor order defines canonical selection order (`Optimization::ALL`), not an
implicit optimization level or pass schedule. Build preludes and release
rollback tooling consume this generated table directly so no second list of
names or tags can drift out of sync.
*/

/*
Phase projection (`project_phase`, `project_psi`, `project_post_terminal`)
routes a stage's local subset of optimizations while retaining the full
`OptimizationSelectionIdentity` of the complete authored policy:
- `Psi` executes before sealed Terminal Psi publication; its rows project
  1-to-1 into `optimization::PsiOptimization` via `psi_projection_value!`,
  while `From<optimization::PsiOptimization>` is checked exhaustively by the
  compiler in the reverse direction. Phase discriminant 1 is retired and is
  not reused.
- `PostTerminalOptimizationSelections` (`post_terminal.rs`) admits only phases
  where `is_post_terminal()` holds (`AbstractOperations` through
  `FunctionRelativeLayout`), making pre-Terminal selections unrepresentable in
  post-Terminal native lowering.
- `CountFrame` and the 40-byte five-counter helpers (`encode_work_counters`,
  `decode_work_counters`) are shared here so wire records (`u32` counts) and
  canonical identity rows (`u64` counts) serialize their fields through a single
  definition.
*/

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

pub(crate) fn encode_work_counters(fields: [u64; 5]) -> [u8; 40] {
    let mut encoded = [0; 40];
    for (index, value) in fields.into_iter().enumerate() {
        crate::codec_bytes::patch_encoded(&mut encoded, index * 8, value.to_le_bytes());
    }
    encoded
}

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
const SELECTION_ENCODING_VERSION: u32 = 16;
const SELECTION_IDENTITY_DOMAIN: &[u8] = b"omega.optimization-selections.v16\0";

crate::tagged_vocabulary! {
    8;
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum OptimizationExecutionPhase {
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
    pub const fn is_post_terminal(self) -> bool {
        !matches!(self, Self::Psi)
    }
}

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

optimization_vocabulary! {
    54;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizationCatalogDescriptor<Payload> {
    optimization: Optimization, // Authoritative source-visible optimization family.
    payload: Payload, // Stage-owned descriptor payload (target predicates, constructors, validators).
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OptimizationSelections {
    selected: Vec<Optimization>, // Strictly ascending, duplicate-free list in canonical tag order.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizationPhaseSelections {
    phase: OptimizationExecutionPhase, // Execution phase this projection targets.
    complete_selection: OptimizationSelectionIdentity, // Digest of the full authored selection set.
    selected: OptimizationSelections,  // Subset of optimizations routed to `phase`.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizationPhaseMismatch {
    pub expected: OptimizationExecutionPhase,
    pub actual: OptimizationExecutionPhase,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiOptimizationSelectionProjection {
    complete_selection: OptimizationSelectionIdentity, // Digest of the full authored selection set.
    selected: optimization::PsiOptimizationSelections, // Target-neutral Psi-phase subset.
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

    pub fn project_phase(&self, phase: OptimizationExecutionPhase) -> OptimizationPhaseSelections {
        OptimizationPhaseSelections {
            phase,
            complete_selection: self.identity(),
            selected: self.for_phase(phase),
        }
    }

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

pub use crate::digest_encoder::hex;

pub fn encode_option_u64(bytes: &mut Vec<u8>, value: Option<u64>) {
    match value {
        Some(value) => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
        }
        None => bytes.push(0),
    }
}
