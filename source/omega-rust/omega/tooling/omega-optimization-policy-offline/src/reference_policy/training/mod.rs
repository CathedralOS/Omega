//! Optimizer module role: executable entrance. CostThresholdV1 reference training.
//!
//! `compute` selects the deterministic threshold from the validated training
//! split. `replay` independently reconstructs the complete model before this
//! entrance returns custody.

mod compute;
pub(super) mod replay;

use crate::ValidatedOfflinePolicyCorpus;

use super::model::{CostThresholdV1Model, OfflinePolicyReferenceError};

pub(super) fn train(
    corpus: &ValidatedOfflinePolicyCorpus,
) -> Result<CostThresholdV1Model, OfflinePolicyReferenceError> {
    let model = compute::compute(corpus)?;
    replay::validate(&model, corpus)?;
    Ok(model)
}

pub(super) fn validate(
    model: &CostThresholdV1Model,
    corpus: &ValidatedOfflinePolicyCorpus,
) -> Result<(), OfflinePolicyReferenceError> {
    replay::validate(model, corpus)
}
