//! Optimizer module role: carrier leaf. Immutable exact relocation proposal and accepted output.

use super::{
    BlockId, CountdownInvariantConstantAnalysisError,
    CountdownInvariantConstantPlacementAnalysisError, CountedLoopAnalysisError, CycleComponentId,
    MachineId, NodeLocation, OperationId, OptimizationCandidateIdentity, OptimizationUnitIdentity,
    PlaceId, ProvenanceRewrite, PsiOptimizationUnit, PsiProvenance, PsiTransformationLedger,
    ScalarType, ValueId, VerifiedPsiOptimizationSession,
};
/// The result one relocated node preserves. Scalar-constant leaves, admitted
/// place observations, invariant scalar computations, scalar-signature
/// calls, and scalar-result structural calls keep the one scalar definition
/// the node carried — the value downstream uses and run-internal
/// operands stay bound to. A `ByteSequenceSubslice` defines no scalar: the
/// relocation instead preserves the structural view result its operation
/// spells — the fresh root's place, type, multiplicity, and qualifications
/// stay byte-exact inside the moved operation rather than re-spelling a
/// scalar result. An `EstablishPrimitiveLocal` defines no scalar either: the
/// relocation preserves the declared primitive-local result its operation
/// spells — the fresh storage cell's place, type, multiplicity, and
/// claim-free custody stay byte-exact inside the moved operation, so a
/// `CallStructuralScalar` borrowing that root relocates in the same run
/// without re-spelling the argument. An `EstablishByteSequenceLiteral`
/// defines no scalar
/// either: the relocation preserves the literal place declaration its
/// operation carries — the fresh immutable view root's identity and
/// declaration kind stay byte-exact inside the moved operation. A `CallUnit`
/// defines no scalar and establishes no place: the relocation preserves the
/// invocation itself — callee, scalar and structural arguments, claims, and
/// crash routes move byte-exact apart from the planned operand and
/// argument-root rebinds — so the transformed unit's call custody still sees
/// the same invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopInvariantNodeResult {
    /// A preserved scalar definition: the result value and its declared type.
    Scalar {
        value: ValueId,
        scalar_type: ScalarType,
    },
    /// A preserved structural operation result — the fresh view place a
    /// `ByteSequenceSubslice` establishes, the fresh storage cell an
    /// `EstablishPrimitiveLocal` declares, the fresh payload an
    /// `EstablishScalarArray` or `EstablishRecord` binds, the fresh sum
    /// place an `EstablishScalarCase` declares, or the fresh place a
    /// `CallStructural` returns. The moved operation keeps it
    /// byte-exact, so the transformed unit's structural custody still sees
    /// the same producer declaring the same place — for an affine case or
    /// call
    /// result the realization additionally re-spells where member edges and
    /// returns dispose that persistent place.
    Structural(terminal_psi::StructuralOperationResult),
    /// A preserved literal place declaration — the fresh immutable view root
    /// an `EstablishByteSequenceLiteral` establishes. The moved operation
    /// keeps the declaration and payload byte-exact, so the transformed
    /// unit's structural custody still sees the same producer declaring the
    /// same place and every consumer keeps spelling the same place identity.
    LiteralPlace(terminal_psi::StructuralPlaceDeclaration),
    /// A preserved unit-result invocation — an admitted `CallUnit`. The
    /// moved operation defines nothing and establishes nothing: the
    /// relocation rebinds its member-parameter scalar operands and
    /// structural-argument roots and keeps every other field byte-exact, so
    /// the transformed unit still observes the same callee invoked on
    /// equivalent arguments.
    Unit,
}

impl LoopInvariantNodeResult {
    /// The scalar value this result preserves — `None` for a
    /// structural-producing, place-declaring, or unit-result relocation,
    /// which defines no scalar.
    pub const fn scalar_value(&self) -> Option<ValueId> {
        match self {
            Self::Scalar { value, .. } => Some(*value),
            Self::Structural(_) | Self::LiteralPlace(_) | Self::Unit => None,
        }
    }
}

/// One loop-invariant scalar node selected for relocation. The node records
/// the exact source-owned custody the ledger and validator must see: its
/// operation identity, preserved result, original location, provenance, and
/// fuel settlements, plus the operand rebinding an invariant member parameter
/// performs when the computation is re-expressed on its preheader-visible
/// representative. Scalar-constant leaves and admitted place observations
/// carry an empty operand rewrite list — an observation whose storage root
/// already names a preheader-visible place or a member-produced root the
/// same run covers relocates byte-exact like a leaf,
/// while one reading through an invariant member structural parameter records
/// that root rebind in `root_rewrite`. A `ByteSequenceSubslice` records the
/// same root rebind and scalar-operand rewrites a byte read does, but its
/// result is [`LoopInvariantNodeResult::Structural`]: the fresh view stays
/// byte-exact inside the moved operation. An `EstablishPrimitiveLocal`
/// carries that same `Structural` result — the declared cell stays
/// byte-exact — while its scalar initializer records an ordinary
/// `operand_rewrites` entry. An admitted `CallUnit` records its
/// result as [`LoopInvariantNodeResult::Unit`], rebinds member-parameter
/// scalar operands through `operand_rewrites` like every computation, and
/// rebinds each structural argument whose root is an invariant member
/// parameter through `argument_rewrites` — a borrow or copyable-owned
/// argument naming a root a node earlier in the same run produced needs no
/// rewrite, because the run keeps the producer's declared place identity
/// byte-exact. An
/// admitted `CallStructuralScalar` carries the same operand and
/// argument-root rewrites while recording its result as
/// [`LoopInvariantNodeResult::Scalar`]: the relocated call's return value
/// stays bound for the member consumers the same run relocates. An
/// admitted `CallStructural` records its affine result as
/// [`LoopInvariantNodeResult::Structural`] — the declared place stays
/// byte-exact while the realization re-spells its dispatch custody — and
/// carries the same operand and argument-root rewrites a
/// `CallStructuralScalar` does: scalar `arguments` through
/// `operand_rewrites` and each structural argument whose root is an
/// invariant member structural parameter through `argument_rewrites`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopInvariantScalarNode {
    pub(super) psi_operation: OperationId,
    pub(super) result: LoopInvariantNodeResult,
    pub(super) location: NodeLocation,
    pub(super) operand_rewrites: Vec<(ValueId, ValueId)>,
    /// The observed-root rebind an admitted place observation performs when
    /// its source names an invariant member structural parameter: `(member
    /// parameter root, entry representative root)`. `None` for scalar-constant
    /// leaves, scalar computations, and observations whose root is already
    /// preheader-visible or produced by a node the same run covers.
    pub(super) root_rewrite: Option<(PlaceId, PlaceId)>,
    /// The structural-argument root rebinds an admitted call performs:
    /// `(member parameter root, preheader-visible root)` pairs for every
    /// argument whose root is an invariant member structural parameter.
    /// Empty for every non-call relocation and for a call whose argument
    /// roots are already preheader-visible or produced inside the same run.
    pub(super) argument_rewrites: Vec<(PlaceId, PlaceId)>,
    pub(super) provenance: Vec<PsiProvenance>,
    pub(super) fuel: Vec<optimization_unit::FuelSettlement>,
}

impl LoopInvariantScalarNode {
    pub const fn psi_operation(&self) -> OperationId {
        self.psi_operation
    }

    pub const fn result(&self) -> &LoopInvariantNodeResult {
        &self.result
    }

    pub const fn location(&self) -> NodeLocation {
        self.location
    }

    /// Exact `(invariant parameter, entry representative)` operand rewrites
    /// the relocation performs, sorted by parameter. Empty for scalar-constant
    /// leaves, which read no values, for admitted place observations, which
    /// rebind their storage root through `root_rewrite` instead, and for
    /// chained computations whose member-internal operands all name results
    /// the same run preserves.
    pub fn operand_rewrites(&self) -> &[(ValueId, ValueId)] {
        &self.operand_rewrites
    }

    /// The `(member structural parameter, entry representative)` storage-root
    /// rebind an admitted place observation performs, when its source names a
    /// member parameter every reaching edge resolves to the same
    /// preheader-visible or run-covered root. `None` for every other
    /// relocated node.
    pub const fn root_rewrite(&self) -> Option<(PlaceId, PlaceId)> {
        self.root_rewrite
    }

    /// The `(member parameter root, preheader-visible root)` rewrites an
    /// admitted call performs on its structural-argument roots, in argument
    /// order. Empty for every non-call relocation and for a call whose
    /// argument roots are already preheader-visible or produced inside the
    /// same run.
    pub fn argument_rewrites(&self) -> &[(PlaceId, PlaceId)] {
        &self.argument_rewrites
    }

    pub fn provenance(&self) -> &[PsiProvenance] {
        &self.provenance
    }

    pub fn fuel(&self) -> &[optimization_unit::FuelSettlement] {
        &self.fuel
    }
}

/// One scalar relocation: the node's exact source node and its destination
/// coordinate inside the component's unique preheader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopInvariantScalarRelocation {
    pub(super) node: LoopInvariantScalarNode,
    pub(super) destination: NodeLocation,
}

impl LoopInvariantScalarRelocation {
    pub const fn node(&self) -> &LoopInvariantScalarNode {
        &self.node
    }

    pub const fn destination(&self) -> NodeLocation {
        self.destination
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopInvariantScalarMotionCandidate {
    pub(super) identity: OptimizationCandidateIdentity,
    pub(super) input: OptimizationUnitIdentity,
    pub(super) output: OptimizationUnitIdentity,
    pub(super) component: CycleComponentId,
    pub(super) relocations: Vec<LoopInvariantScalarRelocation>,
}

impl LoopInvariantScalarMotionCandidate {
    pub const fn identity(&self) -> OptimizationCandidateIdentity {
        self.identity
    }

    pub const fn input(&self) -> OptimizationUnitIdentity {
        self.input
    }

    pub const fn output(&self) -> OptimizationUnitIdentity {
        self.output
    }

    pub const fn component(&self) -> &CycleComponentId {
        &self.component
    }

    pub fn relocations(&self) -> &[LoopInvariantScalarRelocation] {
        &self.relocations
    }
}

#[derive(Debug)]
pub struct ValidatedLoopInvariantScalarMotion {
    pub(super) candidate: LoopInvariantScalarMotionCandidate,
    pub(super) output: PsiOptimizationUnit,
    pub(super) provenance: Vec<ProvenanceRewrite>,
}

impl ValidatedLoopInvariantScalarMotion {
    pub const fn candidate(&self) -> &LoopInvariantScalarMotionCandidate {
        &self.candidate
    }

    pub fn provenance(&self) -> &[ProvenanceRewrite] {
        &self.provenance
    }
}

#[derive(Debug)]
pub struct AppliedLoopInvariantScalarMotion {
    pub(super) session: VerifiedPsiOptimizationSession,
    pub(super) candidate: LoopInvariantScalarMotionCandidate,
    pub(super) ledger: PsiTransformationLedger,
}

impl AppliedLoopInvariantScalarMotion {
    pub const fn session(&self) -> &VerifiedPsiOptimizationSession {
        &self.session
    }

    pub const fn candidate(&self) -> &LoopInvariantScalarMotionCandidate {
        &self.candidate
    }

    pub const fn ledger(&self) -> &PsiTransformationLedger {
        &self.ledger
    }

    pub fn into_session(self) -> VerifiedPsiOptimizationSession {
        self.session
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopInvariantScalarMotionError {
    CandidateBudgetExhausted {
        required: u64,
        limit: u64,
    },
    StaleCandidateRevision {
        candidate: OptimizationUnitIdentity,
        current: OptimizationUnitIdentity,
    },
    UnknownComponent,
    AlreadyRelocated,
    CandidateMismatch,
    MissingNode {
        machine: MachineId,
        block: BlockId,
        node: u32,
    },
    CoordinateOverflow,
    OutputIdentityMismatch {
        candidate: OptimizationUnitIdentity,
        reconstructed: OptimizationUnitIdentity,
    },
    TransformedValidation(optimization_unit_semantics::OptimizationUnitValidationError),
    CountedLoop(CountedLoopAnalysisError),
    InvariantConstant(CountdownInvariantConstantAnalysisError),
    ReconstructedPlacement(CountdownInvariantConstantPlacementAnalysisError),
    InvalidLedger(optimization_unit::InvalidPsiTransformationLedger),
}

impl std::fmt::Display for LoopInvariantScalarMotionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "loop-invariant scalar motion failure: {self:?}")
    }
}

impl std::error::Error for LoopInvariantScalarMotionError {}

pub(super) fn candidate_identity(
    input: OptimizationUnitIdentity,
    output: OptimizationUnitIdentity,
    component: &CycleComponentId,
    relocations: &[LoopInvariantScalarRelocation],
) -> OptimizationCandidateIdentity {
    let mut canonical = b"omega.psi.loop-invariant-scalar-motion-candidate.v1".to_vec();
    canonical.extend_from_slice(&input.bytes());
    canonical.extend_from_slice(&output.bytes());
    canonical.extend_from_slice(&component.machine.get().to_le_bytes());
    canonical.extend_from_slice(
        &u64::try_from(component.internal_edges.len())
            .expect("component edge count fits u64")
            .to_le_bytes(),
    );
    for edge in &component.internal_edges {
        canonical.extend_from_slice(&edge.edge.get().to_le_bytes());
        canonical.extend_from_slice(&edge.source.get().to_le_bytes());
        canonical.extend_from_slice(&edge.target.get().to_le_bytes());
    }
    canonical.extend_from_slice(
        &u64::try_from(relocations.len())
            .expect("relocation count fits u64")
            .to_le_bytes(),
    );
    for relocation in relocations {
        canonical.extend_from_slice(&relocation.node.psi_operation.get().to_le_bytes());
        match &relocation.node.result {
            LoopInvariantNodeResult::Scalar { value, .. } => {
                canonical.push(0);
                canonical.extend_from_slice(&value.get().to_le_bytes());
            }
            LoopInvariantNodeResult::Structural(result) => {
                canonical.push(1);
                canonical.extend_from_slice(&result.place.get().to_le_bytes());
                canonical.extend_from_slice(&result.structural_type.get().to_le_bytes());
                canonical.push(match result.multiplicity {
                    terminal_psi::StructuralMultiplicity::Unrestricted => 0,
                    terminal_psi::StructuralMultiplicity::Affine => 1,
                    terminal_psi::StructuralMultiplicity::Linear => 2,
                });
                // Admitted structural results carry no qualification or claim
                // rows; the roster lengths still commit the claim so a richer
                // shape can never collide with an admitted one. The output
                // unit identity binds their full contents byte-exact.
                for count in [
                    result.qualifications.len(),
                    result.projected_qualifications.len(),
                    result.claims.len(),
                ] {
                    canonical.extend_from_slice(
                        &u64::try_from(count)
                            .expect("result roster length fits u64")
                            .to_le_bytes(),
                    );
                }
            }
            LoopInvariantNodeResult::LiteralPlace(place) => {
                canonical.push(2);
                canonical.extend_from_slice(&place.id.get().to_le_bytes());
                match place.kind {
                    semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                        declaration_ordinal,
                        structural_type,
                    } => {
                        canonical.push(0);
                        canonical.extend_from_slice(&declaration_ordinal.to_le_bytes());
                        canonical.extend_from_slice(&structural_type.get().to_le_bytes());
                    }
                    // Admission only produces a byte-sequence-literal
                    // declaration; any other kind still commits byte-exact
                    // through the output unit identity.
                    _ => canonical.push(1),
                }
            }
            LoopInvariantNodeResult::Unit => {
                // A preserved invocation commits nothing beyond the tag: the
                // callee, arguments, claims, and crash routes move byte-exact
                // inside the output unit identity, and the rebinds below
                // record the only fields that change.
                canonical.push(3);
            }
        }
        encode_location(&mut canonical, relocation.node.location);
        encode_location(&mut canonical, relocation.destination);
        canonical.extend_from_slice(
            &u64::try_from(relocation.node.operand_rewrites.len())
                .expect("operand rewrite count fits u64")
                .to_le_bytes(),
        );
        for (parameter, representative) in &relocation.node.operand_rewrites {
            canonical.extend_from_slice(&parameter.get().to_le_bytes());
            canonical.extend_from_slice(&representative.get().to_le_bytes());
        }
        match relocation.node.root_rewrite {
            Some((parameter, representative)) => {
                canonical.push(1);
                canonical.extend_from_slice(&parameter.get().to_le_bytes());
                canonical.extend_from_slice(&representative.get().to_le_bytes());
            }
            None => canonical.push(0),
        }
        canonical.extend_from_slice(
            &u64::try_from(relocation.node.argument_rewrites.len())
                .expect("argument rewrite count fits u64")
                .to_le_bytes(),
        );
        for (parameter, root) in &relocation.node.argument_rewrites {
            canonical.extend_from_slice(&parameter.get().to_le_bytes());
            canonical.extend_from_slice(&root.get().to_le_bytes());
        }
    }
    OptimizationCandidateIdentity::from_canonical_bytes(&canonical)
}

fn encode_location(canonical: &mut Vec<u8>, location: NodeLocation) {
    canonical.extend_from_slice(&location.machine.get().to_le_bytes());
    canonical.extend_from_slice(&location.block.get().to_le_bytes());
    canonical.extend_from_slice(&location.node.to_le_bytes());
}
