use crate::{OfflinePolicySplit, ValidatedOfflinePolicyCorpus};

use super::super::{
    identity::{
        cost_threshold_v1_algorithm_identity, offline_policy_split_identity, report_identity,
    },
    model::{
        CostThresholdV1Model, OfflinePolicyEvaluationReport, OfflinePolicyPrediction,
        OfflinePolicyReferenceError,
    },
    training::replay::{replay_prediction, replay_summary},
};

pub(super) fn validate(
    report: &OfflinePolicyEvaluationReport,
    corpus: &ValidatedOfflinePolicyCorpus,
    model: &CostThresholdV1Model,
) -> Result<(), OfflinePolicyReferenceError> {
    if report.corpus != corpus.identity() {
        return Err(OfflinePolicyReferenceError::WrongCorpus);
    }
    if report.model != model.identity {
        return Err(OfflinePolicyReferenceError::WrongModel);
    }
    if report.algorithm != cost_threshold_v1_algorithm_identity()
        || report.algorithm != model.algorithm
    {
        return Err(OfflinePolicyReferenceError::WrongAlgorithm);
    }
    if !matches!(
        report.split,
        OfflinePolicySplit::Evaluation | OfflinePolicySplit::Regression
    ) {
        return Err(OfflinePolicyReferenceError::UnsupportedReportSplit(
            report.split,
        ));
    }
    if report.split_identity != offline_policy_split_identity(corpus, report.split) {
        return Err(OfflinePolicyReferenceError::ReportMismatch);
    }
    let examples = corpus
        .examples()
        .iter()
        .filter(|example| example.split() == report.split)
        .collect::<Vec<_>>();
    if examples.is_empty() {
        return Err(OfflinePolicyReferenceError::EmptySplit(report.split));
    }
    let predictions = examples
        .iter()
        .map(|example| {
            let (action, selected_predicted_cost_delta) =
                replay_prediction(example.point(), model.threshold);
            OfflinePolicyPrediction {
                surface: example.surface(),
                action,
                selected_predicted_cost_delta,
            }
        })
        .collect::<Vec<_>>();
    if report.predictions != predictions {
        return Err(OfflinePolicyReferenceError::NonCanonicalPredictions);
    }
    let summary = replay_summary(&examples, model.threshold)?;
    if report.summary != summary {
        return Err(OfflinePolicyReferenceError::ReportMismatch);
    }
    if report.identity != report_identity(report) {
        return Err(OfflinePolicyReferenceError::ReportIdentityMismatch);
    }
    Ok(())
}
