//! Optimizer module role: executable entrance. Exact constant state-argument dispatch specialization boundary.
//!
//! One bounded specialization family: a dispatch state is a non-entry block
//! whose only node is a `Conditional` on one of that block's own scalar
//! parameters — the state argument. When an unconditional `Jump` edge entering
//! the dispatch binds that parameter to an argument the sparse conditional
//! constant analysis proves is one exact Boolean, the traversal "this edge,
//! then the dispatch's resolved arm" specializes into a single edge: the
//! incoming edge is retargeted to the resolved arm's block with its scalar
//! bindings composed through the edge's own bindings, and the fused edge
//! carries both source edges' custody — its own `PsiProvenance::Edge` first,
//! then the resolved arm's — with one paired fuel settlement each. The
//! dispatch state and both of its arm edges remain for every other incoming
//! edge, so no source work is removed and no block becomes unreachable.
//!
//! Only machines absent from the authenticated Terminal-cycle component roster
//! are eligible: a machine containing a verified cyclic component is frozen
//! byte-exact under `validate_frozen_component_blocks`, so edge surgery there
//! can never be admitted. Proposal, independent validation, and application
//! are separate boundaries; the candidate pins the exact input and output
//! revision identities, the supplying edge identity, the resolved arm edge,
//! and the proven constant, so replay rejects forged or mismatched edge
//! provenance by recomputation rather than trust.

use optimization_core::{
    OptimizationCandidateIdentity, OptimizationRuleIdentity, OptimizationUnitIdentity,
    OptimizationValidatorIdentity,
};
use optimization_unit::{
    NodeLocation, OptimizationEdge, OptimizationNode, ProvenanceDisposition, ProvenanceRewrite,
    PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance, PsiRealizationSite,
    PsiTransformationLedger, PsiTransformationRecord, ValueUse,
    recompute_psi_optimization_unit_identity,
};
use semantic_vocabulary::{BlockId, EdgeId, MachineId, ValueId};

use abstract_operations::{AbstractOperation as O, ValueBinding};

use crate::{
    AnalysisProduct, ScalarConstant, ScalarConstantAnalysis, VerifiedPsiOptimizationSession,
    compute_analysis,
};

mod admission;
mod apply;
mod model;
pub(crate) mod propose;
pub(crate) mod validate;

pub use model::{
    AppliedStateArgumentSpecialization, SpecializedStateEdge, StateArgumentSpecializationCandidate,
    StateArgumentSpecializationError, ValidatedStateArgumentSpecialization,
};
use model::{DispatchSpecializationPlan, candidate_identity};

pub fn propose_state_argument_specializations(
    session: &VerifiedPsiOptimizationSession,
    candidate_limit: u64,
) -> Result<Vec<StateArgumentSpecializationCandidate>, StateArgumentSpecializationError> {
    propose::all(session, candidate_limit)
}

pub fn validate_state_argument_specialization(
    session: &VerifiedPsiOptimizationSession,
    candidate: &StateArgumentSpecializationCandidate,
) -> Result<ValidatedStateArgumentSpecialization, StateArgumentSpecializationError> {
    validate::candidate(session, candidate)
}

pub fn apply_state_argument_specialization(
    session: VerifiedPsiOptimizationSession,
    validated: ValidatedStateArgumentSpecialization,
) -> Result<AppliedStateArgumentSpecialization, StateArgumentSpecializationError> {
    apply::validated(session, validated)
}

fn rule_identity() -> OptimizationRuleIdentity {
    OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.state-argument-specialization.v1",
    )
}

fn validator_identity() -> OptimizationValidatorIdentity {
    OptimizationValidatorIdentity::from_canonical_bytes(
        b"omega.psi-validator.state-argument-specialization.v1",
    )
}

#[cfg(test)]
mod tests;
