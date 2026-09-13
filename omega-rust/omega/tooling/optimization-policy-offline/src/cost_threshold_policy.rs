//! Optimizer module role: executable entrance. CostThresholdV1 policy lifecycle.
//!
//! Fit a threshold, independently replay the model, then evaluate a held-out
//! split and replay its report before returning it. The algorithms and their
//! independent checks live beneath this owner. These offline results grant no
//! compiler activation, rewrite, process, publication, or quality authority.

mod codec;
mod evaluation;
mod identity;
mod inference;
mod model;
mod regression_manifest;
mod training;

#[cfg(test)]
mod tests;

pub use codec::{
    decode_model as decode_cost_threshold_v1_model,
    decode_report as decode_cost_threshold_v1_report,
};
pub use identity::{
    OfflinePolicyAlgorithmIdentity, OfflinePolicyModelIdentity, OfflinePolicyReportIdentity,
    OfflinePolicySplitIdentity, cost_threshold_v1_algorithm_identity,
    offline_policy_split_identity,
};
pub use model::{
    CostThresholdV1Model, OfflinePolicyConfusion, OfflinePolicyEvaluationReport,
    OfflinePolicyEvaluationSummary, OfflinePolicyPrediction, OfflinePolicyReferenceError,
};
pub use regression_manifest::{
    OfflinePolicyRegressionManifest, OfflinePolicyRegressionManifestIdentity,
    create as create_cost_threshold_v1_regression_manifest,
    decode as decode_cost_threshold_v1_regression_manifest,
};

use crate::{OfflinePolicySplit, ValidatedOfflinePolicyCorpus};

/// Fit the training split and independently replay the complete model before returning it.
pub fn train_cost_threshold_v1(
    corpus: &ValidatedOfflinePolicyCorpus,
) -> Result<CostThresholdV1Model, OfflinePolicyReferenceError> {
    let model = training::fit_threshold(corpus)?;
    training::replay::validate(&model, corpus)?;
    Ok(model)
}

/// Validate model custody, then predict and replay an evaluation or regression split.
/// Training data cannot be requested as a held-out report.
pub fn evaluate_cost_threshold_v1(
    corpus: &ValidatedOfflinePolicyCorpus,
    model: &CostThresholdV1Model,
    split: OfflinePolicySplit,
) -> Result<OfflinePolicyEvaluationReport, OfflinePolicyReferenceError> {
    training::replay::validate(model, corpus)?;
    if !matches!(
        split,
        OfflinePolicySplit::Evaluation | OfflinePolicySplit::Regression
    ) {
        return Err(OfflinePolicyReferenceError::UnsupportedReportSplit(split));
    }
    let report = evaluation::predict_split(corpus, model, split)?;
    evaluation::replay::validate(&report, corpus, model)?;
    Ok(report)
}
