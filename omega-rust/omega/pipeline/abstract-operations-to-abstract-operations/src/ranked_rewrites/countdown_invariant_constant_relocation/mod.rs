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
mod model;
mod propose;
mod validate;

use model::candidate_identity;
pub use model::{
    AppliedCountdownInvariantConstantRelocation, CountdownInvariantConstantRelocation,
    CountdownInvariantConstantRelocationCandidate, CountdownInvariantConstantRelocationError,
    ValidatedCountdownInvariantConstantRelocation,
};

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
