//! Invariants shared by every rewrite patch family.

use std::collections::BTreeSet;

use super::super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate(
    contract: &OptimizationRuleContract,
    decision_point: &PsiRewriteDecisionPoint,
    affected_blocks: &[BlockId],
    substitutions: &[ScalarSubstitution],
    provenance: &[ProvenanceRewrite],
    witness: &PsiRewriteWitness,
) -> Result<Option<NodeLocation>, PsiRewriteCandidateError> {
    let location = match &decision_point {
        PsiRewriteDecisionPoint::Node(location) => Some(*location),
        PsiRewriteDecisionPoint::MachineSet(_) => None,
    };
    if affected_blocks.is_empty() && location.is_some() {
        return Err(PsiRewriteCandidateError::EmptyAffectedRegion);
    }
    if affected_blocks.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(PsiRewriteCandidateError::NonCanonicalAffectedRegion);
    }
    if location.is_some_and(|location| !affected_blocks.contains(&location.block)) {
        return Err(PsiRewriteCandidateError::DecisionPointOutsideRegion);
    }
    if substitutions.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(PsiRewriteCandidateError::NonCanonicalSubstitutions);
    }
    if provenance.is_empty() || provenance.iter().any(|row| row.sources.is_empty()) {
        return Err(PsiRewriteCandidateError::EmptyProvenanceSource);
    }
    if provenance.windows(2).any(|pair| {
        let left = (
            pair[0].input,
            pair[0].disposition.canonical_tag(),
            pair[0].disposition.site(),
        );
        let right = (
            pair[1].input,
            pair[1].disposition.canonical_tag(),
            pair[1].disposition.site(),
        );
        left >= right
    }) || provenance
        .iter()
        .any(|row| row.sources.iter().copied().collect::<BTreeSet<_>>().len() != row.sources.len())
    {
        return Err(PsiRewriteCandidateError::NonCanonicalProvenance);
    }
    for group in provenance.chunk_by(|left, right| left.input == right.input) {
        if group.len() > 1
            && (group.iter().any(|row| !row.disposition.is_realized())
                || group
                    .iter()
                    .skip(1)
                    .any(|row| row.sources != group[0].sources || row.fuel != group[0].fuel))
        {
            return Err(PsiRewriteCandidateError::NonCanonicalProvenance);
        }
    }
    for row in provenance {
        let sources = row.sources.iter().copied().collect::<BTreeSet<_>>();
        if row.input.machine() != row.disposition.site().machine() {
            return Err(PsiRewriteCandidateError::NonCanonicalProvenance);
        }
        let fuel = row
            .fuel
            .iter()
            .map(|settlement| settlement.site)
            .collect::<BTreeSet<_>>();
        if fuel.len() != row.fuel.len()
            || fuel != sources
            || row.fuel.iter().any(|settlement| settlement.units == 0)
        {
            return Err(PsiRewriteCandidateError::FuelProvenanceMismatch);
        }
    }
    if matches!(
        contract.safety_class(),
        OptimizationSafetyClass::ProofCertified
    ) != matches!(
        witness,
        PsiRewriteWitness::ScalarEvaluation(
            ScalarEvaluationWitness::ProofCertifiedUnary { .. }
                | ScalarEvaluationWitness::ProofCertifiedBinary { .. }
                | ScalarEvaluationWitness::RangeAgainstConstant { .. }
                | ScalarEvaluationWitness::RangeAgainstRange { .. }
        ) | PsiRewriteWitness::AcceptedObligation(_)
            | PsiRewriteWitness::ProofCertifiedScalarIdentity { .. }
    ) {
        return Err(PsiRewriteCandidateError::ProofWitnessSafetyMismatch);
    }
    Ok(location)
}
