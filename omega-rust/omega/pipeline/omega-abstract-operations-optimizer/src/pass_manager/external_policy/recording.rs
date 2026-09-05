use omega_optimization_core::{
    BaselineDecisionLog, BaselineDecisionOutcome, ExternalCandidateFeatures, ExternalDecisionLog,
    ExternalDecisionPoint,
};
use omega_optimization_core::{
    OptimizationCandidateVerdict, OptimizationDecisionRecord, OptimizationReasonCode,
    OptimizationRuleSetIdentity,
};

use super::super::{OptimizationRun, OptimizationRunError};
use super::expected_context;

pub(crate) fn external_points_from_manifest_decisions(
    decisions: &BaselineDecisionLog,
    manifest_decisions: &[&OptimizationDecisionRecord],
) -> Result<Vec<ExternalDecisionPoint>, OptimizationRunError> {
    let mut points = Vec::with_capacity(decisions.records.len());
    for record in &decisions.records {
        let mut rule = None;
        let mut features = Vec::with_capacity(record.considered.len());
        for candidate in &record.considered {
            let matching = manifest_decisions
                .iter()
                .copied()
                .filter(|decision| {
                    decision.input() == record.input && decision.candidate() == candidate.candidate
                })
                .collect::<Vec<_>>();
            let [decision] = matching.as_slice() else {
                return Err(OptimizationRunError::ExternalDecisionManifestMismatch);
            };
            if let Some(expected) = rule {
                if decision.rule() != expected {
                    return Err(OptimizationRunError::ExternalDecisionManifestMismatch);
                }
            } else {
                rule = Some(decision.rule());
            }
            let expected_verdict = match record.outcome {
                BaselineDecisionOutcome::Choose(chosen) if chosen == candidate.candidate => {
                    OptimizationCandidateVerdict::Applied
                }
                BaselineDecisionOutcome::Choose(_) => {
                    OptimizationCandidateVerdict::Skipped(OptimizationReasonCode::Superseded)
                }
                BaselineDecisionOutcome::Skip(reason) => {
                    OptimizationCandidateVerdict::Skipped(reason)
                }
            };
            if decision.verdict() != expected_verdict {
                return Err(OptimizationRunError::ExternalDecisionManifestMismatch);
            }
            features.push(
                ExternalCandidateFeatures::new(
                    *candidate,
                    decision.consumed_analyses(),
                    decision.consumed_facts().iter().copied(),
                )
                .map_err(OptimizationRunError::ExternalDecisionSchema)?,
            );
        }
        points.push(
            ExternalDecisionPoint::new(
                record.input,
                rule.ok_or(OptimizationRunError::ExternalDecisionManifestMismatch)?,
                features,
                record.outcome.into(),
            )
            .map_err(OptimizationRunError::ExternalDecisionSchema)?,
        );
    }
    Ok(points)
}

/// Reconstruct the policy surface independently from the baseline log and
/// validated manifests, then compare it with a strict wire round trip.
pub fn validate_external_decision_recording(
    run: &OptimizationRun,
) -> Result<(), OptimizationRunError> {
    let ordered_rules = run
        .pass_manifests()
        .iter()
        .flat_map(|manifest| manifest.ordered_rules().iter().copied())
        .collect::<Vec<_>>();
    let ordered_rule_set = OptimizationRuleSetIdentity::from_ordered_rules(&ordered_rules)
        .map_err(|_| OptimizationRunError::DuplicatePipelineRule)?;
    let manifest_decisions = run
        .pass_manifests()
        .iter()
        .flat_map(|manifest| manifest.decisions())
        .collect::<Vec<_>>();
    let points = external_points_from_manifest_decisions(run.decisions(), &manifest_decisions)?;
    let expected = ExternalDecisionLog::new(
        expected_context(
            run.transformation_ledger().input(),
            run.selections(),
            run.psi_selections(),
            ordered_rule_set,
        ),
        points,
    )
    .map_err(OptimizationRunError::ExternalDecisionSchema)?;
    let decoded = ExternalDecisionLog::decode(&run.external_decisions().encode())
        .map_err(OptimizationRunError::ExternalDecisionSchema)?;
    if decoded != expected {
        return Err(OptimizationRunError::ExternalDecisionManifestMismatch);
    }
    Ok(())
}
