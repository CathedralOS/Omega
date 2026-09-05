//! Canonical corpus admission, reference training, and model publication.

use optimization_policy_offline::train_cost_threshold_v1;

use crate::{
    arguments::TrainingRequest, error::OfflinePolicyCommandError, inputs::read_corpus,
    publication::publish_new,
};

pub(super) fn train(request: TrainingRequest) -> Result<(), OfflinePolicyCommandError> {
    let corpus = read_corpus(&request.corpus)?;
    let model = train_cost_threshold_v1(&corpus)
        .map_err(OfflinePolicyCommandError::InvalidReferenceArtifact)?;
    publish_new(&request.output, &model.encode())
}
