//! Optimizer module role: executable entrance. Offline reference policy.
//!
//! `training` derives one deterministic CostThresholdV1 model from the
//! validated training split. `evaluation` constructs and independently replays
//! reports for evaluation or regression data. Neither path executes a model or
//! grants compiler, optimizer, rewrite, build, process, or quality authority.

mod codec;
mod evaluation;
mod identity;
mod inference;
mod model;
mod training;

#[cfg(test)]
mod tests;

pub use identity::{
    OfflinePolicyAlgorithmIdentity, OfflinePolicyModelIdentity, OfflinePolicyReportIdentity,
    OfflinePolicySplitIdentity, cost_threshold_v1_algorithm_identity,
    offline_policy_split_identity,
};
pub use model::{
    CostThresholdV1Model, OfflinePolicyConfusion, OfflinePolicyEvaluationReport,
    OfflinePolicyEvaluationSummary, OfflinePolicyPrediction, OfflinePolicyReferenceError,
};

use crate::{OfflinePolicySplit, ValidatedOfflinePolicyCorpus};

pub fn train_cost_threshold_v1(
    corpus: &ValidatedOfflinePolicyCorpus,
) -> Result<CostThresholdV1Model, OfflinePolicyReferenceError> {
    training::train(corpus)
}

pub fn evaluate_cost_threshold_v1(
    corpus: &ValidatedOfflinePolicyCorpus,
    model: &CostThresholdV1Model,
    split: OfflinePolicySplit,
) -> Result<OfflinePolicyEvaluationReport, OfflinePolicyReferenceError> {
    evaluation::evaluate(corpus, model, split)
}

pub fn decode_cost_threshold_v1_model(
    encoded: &[u8],
    corpus: &ValidatedOfflinePolicyCorpus,
) -> Result<CostThresholdV1Model, OfflinePolicyReferenceError> {
    codec::decode_model(encoded, corpus)
}

pub fn decode_cost_threshold_v1_report(
    encoded: &[u8],
    corpus: &ValidatedOfflinePolicyCorpus,
    model: &CostThresholdV1Model,
) -> Result<OfflinePolicyEvaluationReport, OfflinePolicyReferenceError> {
    codec::decode_report(encoded, corpus, model)
}
