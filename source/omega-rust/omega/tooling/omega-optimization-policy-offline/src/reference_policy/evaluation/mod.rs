//! Optimizer module role: executable entrance. Offline split evaluation.
//!
//! `compute` constructs the report from a validated model. `replay` derives
//! every prediction and aggregate again from corpus custody before returning.

mod compute;
mod replay;

use crate::{OfflinePolicySplit, ValidatedOfflinePolicyCorpus};

use super::{
    model::{CostThresholdV1Model, OfflinePolicyEvaluationReport, OfflinePolicyReferenceError},
    training,
};

pub(super) fn evaluate(
    corpus: &ValidatedOfflinePolicyCorpus,
    model: &CostThresholdV1Model,
    split: OfflinePolicySplit,
) -> Result<OfflinePolicyEvaluationReport, OfflinePolicyReferenceError> {
    training::validate(model, corpus)?;
    if !matches!(
        split,
        OfflinePolicySplit::Evaluation | OfflinePolicySplit::Regression
    ) {
        return Err(OfflinePolicyReferenceError::UnsupportedReportSplit(split));
    }
    let report = compute::compute(corpus, model, split)?;
    replay::validate(&report, corpus, model)?;
    Ok(report)
}

pub(super) fn validate(
    report: &OfflinePolicyEvaluationReport,
    corpus: &ValidatedOfflinePolicyCorpus,
    model: &CostThresholdV1Model,
) -> Result<(), OfflinePolicyReferenceError> {
    training::validate(model, corpus)?;
    replay::validate(report, corpus, model)
}
