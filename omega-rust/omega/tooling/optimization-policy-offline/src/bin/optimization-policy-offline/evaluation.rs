//! Validated reference-model evaluation and regression report publication.

use optimization_policy_offline::{OfflinePolicySplit, evaluate_cost_threshold_v1};

use crate::{
    arguments::EvaluationRequest,
    error::OfflinePolicyCommandError,
    inputs::{read_corpus, read_model},
    publication::publish_new,
};

pub(super) fn evaluate(request: EvaluationRequest) -> Result<(), OfflinePolicyCommandError> {
    report(request, OfflinePolicySplit::Evaluation)
}

pub(super) fn regression(request: EvaluationRequest) -> Result<(), OfflinePolicyCommandError> {
    report(request, OfflinePolicySplit::Regression)
}

fn report(
    request: EvaluationRequest,
    split: OfflinePolicySplit,
) -> Result<(), OfflinePolicyCommandError> {
    let corpus = read_corpus(&request.corpus)?;
    let model = read_model(&request.model, &corpus)?;
    let report = evaluate_cost_threshold_v1(&corpus, &model, split)
        .map_err(OfflinePolicyCommandError::InvalidReferenceArtifact)?;
    publish_new(&request.output, &report.encode())
}
