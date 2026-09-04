use sha2::{Digest, Sha256};
use std::fmt;

const SELECTION_ENCODING_MAGIC: &[u8; 8] = b"OMGOPT\0\0";
const SELECTION_ENCODING_VERSION: u32 = 15;
const SELECTION_IDENTITY_DOMAIN: &[u8] = b"omega.optimization-selections.v15\0";

/// Closed execution phase for one explicitly named optimization. Phase
/// projection routes a complete source-visible suite; it never replaces that
/// suite's identity or creates an optimization-level alias.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
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

impl OptimizationExecutionPhase {
    /// Canonical pipeline order, including phases whose current exact
    /// selection is necessarily empty. Empty phases still execute as identity
    /// validation boundaries rather than selecting another pipeline.
    pub const ALL: [Self; 9] = [
        Self::CheckedTrees,
        Self::Psi,
        Self::AbstractOperations,
        Self::TargetOperations,
        Self::SelectedLowering,
        Self::PreAllocation,
        Self::AllocationRecovery,
        Self::PostAllocationMachine,
        Self::FunctionRelativeLayout,
    ];
}

macro_rules! optimization_vocabulary {
    (
        $count:literal;
        $(
            $variant:ident = $tag:literal => {
                case: $case:literal,
                counter: $counter:literal,
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

            pub const fn build_counter_field(self) -> &'static str {
                match self { $(Self::$variant => $counter),+ }
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

            fn from_tag(tag: u8) -> Result<Self, SelectionDecodeError> {
                Self::ALL
                    .into_iter()
                    .find(|optimization| *optimization as u8 == tag)
                    .ok_or(SelectionDecodeError::UnknownOptimizationTag(tag))
            }
        }
    };
}

// This is the only declaration of exact names, stable tags, build counters,
// phases, and canonical order. Build preludes are exhaustively checked against
// the generated `ALL`, `build_case_name`, and `build_counter_field` views.
optimization_vocabulary! {
    20;
    ControlFlowCleanup = 1 => {
        case: "ControlFlowCleanup",
        counter: "control_flow_cleanup",
        phase: Psi
    },
    SparseConditionalConstantPropagation = 2 => {
        case: "SparseConditionalConstantPropagation",
        counter: "sparse_conditional_constant_propagation",
        phase: Psi
    },
    CopyPropagation = 3 => {
        case: "CopyPropagation",
        counter: "copy_propagation",
        phase: Psi
    },
    GlobalValueNumbering = 4 => {
        case: "GlobalValueNumbering",
        counter: "global_value_numbering",
        phase: Psi
    },
    DeadPureScalarElimination = 5 => {
        case: "DeadPureScalarElimination",
        counter: "dead_pure_scalar_elimination",
        phase: Psi
    },
    ProofCheckElision = 6 => {
        case: "ProofCheckElision",
        counter: "proof_check_elision",
        phase: Psi
    },
    SelectedIncomingU12ExactAddImmediate = 7 => {
        case: "SelectedIncomingU12ExactAddImmediate",
        counter: "selected_incoming_u12_exact_add_immediate",
        phase: SelectedLowering
    },
    X86RelaxConditionalBranchesToRel8V1 = 8 => {
        case: "X86RelaxConditionalBranchesToRel8V1",
        counter: "x86_relax_conditional_branches_to_rel8_v1",
        phase: FunctionRelativeLayout
    },
    SelectedIncomingU12ExactSubtractImmediate = 9 => {
        case: "SelectedIncomingU12ExactSubtractImmediate",
        counter: "selected_incoming_u12_exact_subtract_immediate",
        phase: SelectedLowering
    },
    Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 = 10 => {
        case: "Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1",
        counter: "aarch64_fuse_compare_i64_zero_branch_nonzero_to_cbnz_v1",
        phase: PostAllocationMachine
    },
    SharedEntryFixedViewCopyAfterCompareBeforeBranchV1 = 11 => {
        case: "SharedEntryFixedViewCopyAfterCompareBeforeBranchV1",
        counter: "shared_entry_fixed_view_copy_after_compare_before_branch_v1",
        phase: AllocationRecovery
    },
    ActiveResidentImmediateU64MultiUseRematerializationV1 = 12 => {
        case: "ActiveResidentImmediateU64MultiUseRematerializationV1",
        counter: "active_resident_immediate_u64_multi_use_rematerialization_v1",
        phase: AllocationRecovery
    },
    Aarch64SelectShortestMovnSeededI64MaterializationV1 = 13 => {
        case: "Aarch64SelectShortestMovnSeededI64MaterializationV1",
        counter: "aarch64_select_shortest_movn_seeded_i64_materialization_v1",
        phase: PostAllocationMachine
    },
    X86SelectXorZeroI64MaterializationV1 = 14 => {
        case: "X86SelectXorZeroI64MaterializationV1",
        counter: "x86_select_xor_zero_i64_materialization_v1",
        phase: PostAllocationMachine
    },
    X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1 = 15 => {
        case: "X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1",
        counter: "x86_select_mov_r32_imm32_zero_extended_i64_materialization_v1",
        phase: PostAllocationMachine
    },
    X86SelectMovR64Imm32SignExtendedI64MaterializationV1 = 16 => {
        case: "X86SelectMovR64Imm32SignExtendedI64MaterializationV1",
        counter: "x86_select_mov_r64_imm32_sign_extended_i64_materialization_v1",
        phase: PostAllocationMachine
    },
    Aarch64ElideSameViewCopyI64BeforeReturnV1 = 17 => {
        case: "Aarch64ElideSameViewCopyI64BeforeReturnV1",
        counter: "aarch64_elide_same_view_copy_i64_before_return_v1",
        phase: PostAllocationMachine
    },
    Aarch64ElideSameViewCopyI64BeforeCompareZeroV1 = 18 => {
        case: "Aarch64ElideSameViewCopyI64BeforeCompareZeroV1",
        counter: "aarch64_elide_same_view_copy_i64_before_compare_zero_v1",
        phase: PostAllocationMachine
    },
    Aarch64ElideSameViewCopyI64BeforeCompareI64LeftOperandV1 = 19 => {
        case: "Aarch64ElideSameViewCopyI64BeforeCompareI64LeftOperandV1",
        counter: "aarch64_elide_same_view_copy_i64_before_compare_i64_left_operand_v1",
        phase: PostAllocationMachine
    },
    Aarch64ElideSameViewCopyI64BeforeCompareI64RightOperandV1 = 20 => {
        case: "Aarch64ElideSameViewCopyI64BeforeCompareI64RightOperandV1",
        counter: "aarch64_elide_same_view_copy_i64_before_compare_i64_right_operand_v1",
        phase: PostAllocationMachine
    },
}

impl Optimization {
    /// Project one unified build-visible selection into Psi's independently
    /// owned pre-Terminal vocabulary. Physical optimizations have no Psi
    /// identity and remain in their Omega-owned phases.
    pub const fn psi_optimization(self) -> Option<psi_optimization::PsiOptimization> {
        use psi_optimization::PsiOptimization;

        match self {
            Self::ControlFlowCleanup => Some(PsiOptimization::ControlFlowCleanup),
            Self::SparseConditionalConstantPropagation => {
                Some(PsiOptimization::SparseConditionalConstantPropagation)
            }
            Self::CopyPropagation => Some(PsiOptimization::CopyPropagation),
            Self::GlobalValueNumbering => Some(PsiOptimization::GlobalValueNumbering),
            Self::DeadPureScalarElimination => Some(PsiOptimization::DeadPureScalarElimination),
            Self::ProofCheckElision => Some(PsiOptimization::ProofCheckElision),
            Self::SelectedIncomingU12ExactAddImmediate
            | Self::X86RelaxConditionalBranchesToRel8V1
            | Self::SelectedIncomingU12ExactSubtractImmediate
            | Self::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1
            | Self::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1
            | Self::ActiveResidentImmediateU64MultiUseRematerializationV1
            | Self::Aarch64SelectShortestMovnSeededI64MaterializationV1
            | Self::X86SelectXorZeroI64MaterializationV1
            | Self::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1
            | Self::X86SelectMovR64Imm32SignExtendedI64MaterializationV1
            | Self::Aarch64ElideSameViewCopyI64BeforeReturnV1
            | Self::Aarch64ElideSameViewCopyI64BeforeCompareZeroV1
            | Self::Aarch64ElideSameViewCopyI64BeforeCompareI64LeftOperandV1
            | Self::Aarch64ElideSameViewCopyI64BeforeCompareI64RightOperandV1 => None,
        }
    }
}

impl From<psi_optimization::PsiOptimization> for Optimization {
    fn from(optimization: psi_optimization::PsiOptimization) -> Self {
        use psi_optimization::PsiOptimization;

        match optimization {
            PsiOptimization::ControlFlowCleanup => Self::ControlFlowCleanup,
            PsiOptimization::SparseConditionalConstantPropagation => {
                Self::SparseConditionalConstantPropagation
            }
            PsiOptimization::CopyPropagation => Self::CopyPropagation,
            PsiOptimization::GlobalValueNumbering => Self::GlobalValueNumbering,
            PsiOptimization::DeadPureScalarElimination => Self::DeadPureScalarElimination,
            PsiOptimization::ProofCheckElision => Self::ProofCheckElision,
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

/// Omega-owned bridge from the complete build selection to the independently
/// encoded target-neutral Psi selection. Coordinators retain the complete
/// identity; Psi consumes only `selected`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiOptimizationSelectionProjection {
    complete_selection: OptimizationSelectionIdentity,
    selected: psi_optimization::PsiOptimizationSelections,
}

/// Omega-owned projection containing only phases that execute after Terminal
/// Psi has been published.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostTerminalOptimizationSelectionProjection {
    complete_selection: OptimizationSelectionIdentity,
    selected: OptimizationSelections,
}

impl PsiOptimizationSelectionProjection {
    pub const fn complete_selection(&self) -> OptimizationSelectionIdentity {
        self.complete_selection
    }

    pub const fn selections(&self) -> &psi_optimization::PsiOptimizationSelections {
        &self.selected
    }
}

impl PostTerminalOptimizationSelectionProjection {
    pub const fn complete_selection(&self) -> OptimizationSelectionIdentity {
        self.complete_selection
    }

    pub const fn selections(&self) -> &OptimizationSelections {
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
        let selected = psi_optimization::PsiOptimizationSelections::new(
            self.selected
                .iter()
                .filter_map(|optimization| optimization.psi_optimization()),
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
            selected: Self {
                selected: self
                    .selected
                    .iter()
                    .copied()
                    .filter(|optimization| match optimization.execution_phase() {
                        OptimizationExecutionPhase::CheckedTrees
                        | OptimizationExecutionPhase::Psi => false,
                        OptimizationExecutionPhase::AbstractOperations
                        | OptimizationExecutionPhase::TargetOperations
                        | OptimizationExecutionPhase::SelectedLowering
                        | OptimizationExecutionPhase::PreAllocation
                        | OptimizationExecutionPhase::AllocationRecovery
                        | OptimizationExecutionPhase::PostAllocationMachine
                        | OptimizationExecutionPhase::FunctionRelativeLayout => true,
                    })
                    .collect(),
            },
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
mod tests;
