use crate::{OfflinePolicySplit, ValidatedOfflinePolicyCorpus};

use super::{identity, model::OfflinePolicyRegressionManifest};
use crate::reference_policy::{
    CostThresholdV1Model, OfflinePolicyReferenceError, cost_threshold_v1_algorithm_identity,
    evaluation, offline_policy_split_identity,
};

pub(super) fn validate(
    manifest: &OfflinePolicyRegressionManifest,
    corpus: &ValidatedOfflinePolicyCorpus,
    model: &CostThresholdV1Model,
) -> Result<(), OfflinePolicyReferenceError> {
    if manifest.corpus != corpus.identity() {
        return Err(OfflinePolicyReferenceError::WrongCorpus);
    }
    if manifest.model != model.identity() {
        return Err(OfflinePolicyReferenceError::WrongModel);
    }
    if manifest.algorithm != cost_threshold_v1_algorithm_identity()
        || manifest.algorithm != model.algorithm()
    {
        return Err(OfflinePolicyReferenceError::WrongAlgorithm);
    }
    if manifest.regression_split
        != offline_policy_split_identity(corpus, OfflinePolicySplit::Regression)
    {
        return Err(OfflinePolicyReferenceError::WrongRegressionSplit);
    }
    let report = evaluation::evaluate(corpus, model, OfflinePolicySplit::Regression)?;
    if manifest.expected_report != report.identity() {
        return Err(OfflinePolicyReferenceError::RegressionReportMismatch);
    }
    if manifest.expected_summary != report.summary() {
        return Err(OfflinePolicyReferenceError::RegressionSummaryMismatch);
    }
    if manifest.identity != identity::identity(manifest) {
        return Err(OfflinePolicyReferenceError::RegressionManifestIdentityMismatch);
    }
    Ok(())
}
