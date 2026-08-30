//! Optimizer module role: executable entrance. Rewrite-candidate construction and admission entrance.
//!
//! Public constructors are grouped into scalar and control-flow families.
//! Every constructor rejoins here: derive one canonical decision point,
//! validate common custody, validate the exact patch family, encode identity,
//! and only then construct the immutable candidate. `access` exposes the
//! admitted value without reopening mutation.

mod access;
mod common_invariants;
mod control_flow;
mod decision_point;
mod patch_invariants;
mod scalar;

use super::codec::encode_candidate;
use super::*;

impl PsiRewriteCandidate {
    #[allow(clippy::too_many_arguments)]
    fn new(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        substitutions: Vec<ScalarSubstitution>,
        provenance: Vec<ProvenanceRewrite>,
        witness: PsiRewriteWitness,
        predicted_cost_delta: i64,
        patch: PsiRewritePatch,
    ) -> Result<Self, PsiRewriteCandidateError> {
        let decision_point = decision_point::derive(&patch)?;
        let location = common_invariants::validate(
            &contract,
            &decision_point,
            &affected_blocks,
            &substitutions,
            &provenance,
            &witness,
        )?;
        patch_invariants::validate(
            location,
            &affected_blocks,
            &substitutions,
            &provenance,
            &witness,
            &patch,
        )?;
        let canonical = encode_candidate(
            input,
            contract,
            &decision_point,
            &affected_blocks,
            &substitutions,
            &provenance,
            &witness,
            predicted_cost_delta,
            &patch,
        );
        let identity = OptimizationCandidateIdentity::from_canonical_bytes(&canonical);
        Ok(Self {
            identity,
            input,
            rule: contract.identity(),
            decision_point,
            affected_blocks,
            required_analyses: contract.required_analyses(),
            invalidated_analyses: contract.invalidated_analyses(),
            safety_class: contract.safety_class(),
            substitutions,
            provenance,
            witness,
            predicted_cost_delta,
            patch,
        })
    }
}
