use optimization_core::{
    BaselineDecisionLog, BaselineDecisionOutcome, ExternalCandidateFeatures,
    ExternalDecisionPoint,
};
use optimization_core::{
    OptimizationCandidateVerdict, OptimizationDecisionRecord, OptimizationReasonCode,
};

use super::super::OptimizationRunError;

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

