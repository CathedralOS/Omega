//! Optimizer module role: carrier leaf. Immutable exact relocation proposal and accepted output.

use super::*;

/// One loop-invariant scalar-constant leaf selected for relocation. The leaf
/// records the exact source-owned custody the ledger and validator must see:
/// its operation identity, defined result, original location, provenance, and
/// fuel settlements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopInvariantScalarLeaf {
    pub(super) psi_operation: OperationId,
    pub(super) result: ValueId,
    pub(super) scalar_type: ScalarType,
    pub(super) location: NodeLocation,
    pub(super) provenance: Vec<PsiProvenance>,
    pub(super) fuel: Vec<optimization_unit::FuelSettlement>,
}

impl LoopInvariantScalarLeaf {
    pub const fn psi_operation(&self) -> OperationId {
        self.psi_operation
    }

    pub const fn result(&self) -> ValueId {
        self.result
    }

    pub const fn scalar_type(&self) -> ScalarType {
        self.scalar_type
    }

    pub const fn location(&self) -> NodeLocation {
        self.location
    }

    pub fn provenance(&self) -> &[PsiProvenance] {
        &self.provenance
    }

    pub fn fuel(&self) -> &[optimization_unit::FuelSettlement] {
        &self.fuel
    }
}

/// One leaf relocation: the leaf's exact source node and its destination
/// coordinate inside the component's unique-entry preheader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopInvariantScalarRelocation {
    pub(super) leaf: LoopInvariantScalarLeaf,
    pub(super) destination: NodeLocation,
}

impl LoopInvariantScalarRelocation {
    pub const fn leaf(&self) -> &LoopInvariantScalarLeaf {
        &self.leaf
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
        canonical.extend_from_slice(&relocation.leaf.psi_operation.get().to_le_bytes());
        canonical.extend_from_slice(&relocation.leaf.result.get().to_le_bytes());
        encode_location(&mut canonical, relocation.leaf.location);
        encode_location(&mut canonical, relocation.destination);
    }
    OptimizationCandidateIdentity::from_canonical_bytes(&canonical)
}

fn encode_location(canonical: &mut Vec<u8>, location: NodeLocation) {
    canonical.extend_from_slice(&location.machine.get().to_le_bytes());
    canonical.extend_from_slice(&location.block.get().to_le_bytes());
    canonical.extend_from_slice(&location.node.to_le_bytes());
}
