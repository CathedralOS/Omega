//! Optimizer module role: executable entrance. Exact authenticated countdown zero/one relocation boundary.

use optimization_core::{
    OptimizationCandidateIdentity, OptimizationRuleIdentity, OptimizationUnitIdentity,
    OptimizationValidatorIdentity,
};
use optimization_unit::CycleComponentId;
use optimization_unit::{
    EffectLink, NodeLocation, OptimizationFact, OptimizationNode, ProvenanceDisposition,
    ProvenanceRewrite, PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance,
    PsiRealizationSite, PsiTransformationLedger, PsiTransformationRecord, ValueDefinitionSite,
    recompute_psi_optimization_unit_identity,
};
use semantic_vocabulary::{BlockId, MachineId, OperationId};

use crate::{
    CountdownInvariantConstantAnalysisError, CountdownInvariantConstantPlacementAnalysisError,
    CountdownInvariantConstantRole, CountdownInvariantIntegerConstant, CountedLoopAnalysisError,
    UnsignedCountdownInvariantConstantPlacements, VerifiedPsiOptimizationSession,
};

mod apply;
mod propose;
mod validate;

pub fn propose_countdown_invariant_constant_relocations(
    session: &VerifiedPsiOptimizationSession,
    candidate_limit: u64,
) -> Result<
    Vec<CountdownInvariantConstantRelocationCandidate>,
    CountdownInvariantConstantRelocationError,
> {
    propose::all(session, candidate_limit)
}

pub fn validate_countdown_invariant_constant_relocation(
    session: &VerifiedPsiOptimizationSession,
    candidate: &CountdownInvariantConstantRelocationCandidate,
) -> Result<ValidatedCountdownInvariantConstantRelocation, CountdownInvariantConstantRelocationError>
{
    validate::candidate(session, candidate)
}

pub fn apply_countdown_invariant_constant_relocation(
    session: VerifiedPsiOptimizationSession,
    validated: ValidatedCountdownInvariantConstantRelocation,
) -> Result<AppliedCountdownInvariantConstantRelocation, CountdownInvariantConstantRelocationError>
{
    apply::validated(session, validated)
}

fn rule_identity() -> OptimizationRuleIdentity {
    OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.countdown-invariant-constant-relocation.v1",
    )
}

fn validator_identity() -> OptimizationValidatorIdentity {
    OptimizationValidatorIdentity::from_canonical_bytes(
        b"omega.psi-validator.countdown-invariant-constant-relocation.v1",
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountdownInvariantConstantRelocation {
    constant: CountdownInvariantIntegerConstant,
    destination: NodeLocation,
}

impl CountdownInvariantConstantRelocation {
    pub const fn constant(&self) -> &CountdownInvariantIntegerConstant {
        &self.constant
    }

    pub const fn destination(&self) -> NodeLocation {
        self.destination
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountdownInvariantConstantRelocationCandidate {
    identity: OptimizationCandidateIdentity,
    input: OptimizationUnitIdentity,
    output: OptimizationUnitIdentity,
    component: CycleComponentId,
    relocations: Vec<CountdownInvariantConstantRelocation>,
}

impl CountdownInvariantConstantRelocationCandidate {
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

    pub fn relocations(&self) -> &[CountdownInvariantConstantRelocation] {
        &self.relocations
    }
}

#[derive(Debug)]
pub struct ValidatedCountdownInvariantConstantRelocation {
    candidate: CountdownInvariantConstantRelocationCandidate,
    output: PsiOptimizationUnit,
    provenance: Vec<ProvenanceRewrite>,
}

impl ValidatedCountdownInvariantConstantRelocation {
    pub const fn candidate(&self) -> &CountdownInvariantConstantRelocationCandidate {
        &self.candidate
    }

    pub fn provenance(&self) -> &[ProvenanceRewrite] {
        &self.provenance
    }
}

#[derive(Debug)]
pub struct AppliedCountdownInvariantConstantRelocation {
    session: VerifiedPsiOptimizationSession,
    candidate: CountdownInvariantConstantRelocationCandidate,
    ledger: PsiTransformationLedger,
}

impl AppliedCountdownInvariantConstantRelocation {
    pub const fn session(&self) -> &VerifiedPsiOptimizationSession {
        &self.session
    }

    pub const fn candidate(&self) -> &CountdownInvariantConstantRelocationCandidate {
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
pub enum CountdownInvariantConstantRelocationError {
    Placement(CountdownInvariantConstantPlacementAnalysisError),
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

impl std::fmt::Display for CountdownInvariantConstantRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "countdown invariant-constant relocation failure: {self:?}"
        )
    }
}

impl std::error::Error for CountdownInvariantConstantRelocationError {}

fn candidate_identity(
    input: OptimizationUnitIdentity,
    output: OptimizationUnitIdentity,
    component: &CycleComponentId,
    relocations: &[CountdownInvariantConstantRelocation],
) -> OptimizationCandidateIdentity {
    let mut canonical = b"omega.psi.countdown-invariant-constant-relocation-candidate.v1".to_vec();
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
        canonical.push(match relocation.constant.role {
            CountdownInvariantConstantRole::PositiveGuardZero => 1,
            CountdownInvariantConstantRole::BackedgeDecrementOne => 2,
        });
        encode_location(&mut canonical, relocation.constant.location);
        encode_location(&mut canonical, relocation.destination);
        canonical.extend_from_slice(&relocation.constant.psi_operation.get().to_le_bytes());
        canonical.extend_from_slice(&relocation.constant.result.get().to_le_bytes());
    }
    OptimizationCandidateIdentity::from_canonical_bytes(&canonical)
}

fn encode_location(canonical: &mut Vec<u8>, location: NodeLocation) {
    canonical.extend_from_slice(&location.machine.get().to_le_bytes());
    canonical.extend_from_slice(&location.block.get().to_le_bytes());
    canonical.extend_from_slice(&location.node.to_le_bytes());
}
