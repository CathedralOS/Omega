//! Pass-manifest codec, rule-set, revision, and ledger replay.

use super::*;

pub(crate) fn validate_manifests(
    manifests: &[OptimizationPassManifestRecord],
    expected_rule_set: OptimizationRuleSetIdentity,
    ledger: &PsiTransformationLedger,
) -> Result<(), OptimizedAbstractPlanProjectionError> {
    let flattened_rules = manifests
        .iter()
        .flat_map(|manifest| manifest.ordered_rules().iter().copied())
        .collect::<Vec<_>>();
    let flattened_rule_set = OptimizationRuleSetIdentity::from_ordered_rules(&flattened_rules)
        .map_err(|_| OptimizedAbstractPlanProjectionError::ManifestRuleSetMismatch)?;
    if flattened_rule_set != expected_rule_set {
        return Err(OptimizedAbstractPlanProjectionError::ManifestRuleSetMismatch);
    }
    if manifests.is_empty() && (!ledger.records().is_empty() || !flattened_rules.is_empty()) {
        return Err(OptimizedAbstractPlanProjectionError::ManifestPresenceMismatch);
    }
    let mut revision = ledger.input();
    let mut ledger_index = 0usize;
    for manifest in manifests {
        if OptimizationPassManifestRecord::decode(&manifest.encode())
            .ok()
            .as_ref()
            != Some(manifest)
        {
            return Err(OptimizedAbstractPlanProjectionError::ManifestCodecMismatch);
        }
        if manifest.input() != revision {
            return Err(OptimizedAbstractPlanProjectionError::ManifestRevisionMismatch);
        }
        let decisions = manifest.decisions();
        let mut decision_index = 0usize;
        while decision_index < decisions.len() {
            let input = decisions[decision_index].input();
            if input != revision {
                return Err(OptimizedAbstractPlanProjectionError::ManifestRevisionMismatch);
            }
            let group_end = decisions[decision_index..]
                .iter()
                .position(|decision| decision.input() != input)
                .map_or(decisions.len(), |offset| decision_index + offset);
            let applied = decisions[decision_index..group_end]
                .iter()
                .filter(|decision| decision.verdict() == OptimizationCandidateVerdict::Applied)
                .collect::<Vec<_>>();
            if applied.len() > 1 {
                return Err(OptimizedAbstractPlanProjectionError::ManifestLedgerMismatch);
            }
            if let Some(decision) = applied.first() {
                let record = ledger
                    .records()
                    .get(ledger_index)
                    .ok_or(OptimizedAbstractPlanProjectionError::ManifestLedgerMismatch)?;
                if decision.input() != record.input
                    || decision.candidate() != record.candidate
                    || decision.rule() != record.rule
                    || decision.validator() != Some(record.validator)
                {
                    return Err(OptimizedAbstractPlanProjectionError::ManifestLedgerMismatch);
                }
                revision = record.output;
                ledger_index += 1;
            }
            decision_index = group_end;
        }
        if manifest.output() != revision {
            return Err(OptimizedAbstractPlanProjectionError::ManifestRevisionMismatch);
        }
    }
    if revision != ledger.output() || ledger_index != ledger.records().len() {
        return Err(OptimizedAbstractPlanProjectionError::ManifestLedgerMismatch);
    }
    Ok(())
}
