//! Optimizer module role: executable entrance. Checked regression baseline.
//!
//! Creation snapshots one validated regression report. Decode independently
//! recomputes that report before returning manifest custody.

mod codec;
mod identity;
mod model;
mod validate;

pub use identity::OfflinePolicyRegressionManifestIdentity;
pub use model::OfflinePolicyRegressionManifest;

use crate::ValidatedOfflinePolicyCorpus;

use super::{CostThresholdV1Model, OfflinePolicyReferenceError};

pub(super) fn create(
    corpus: &ValidatedOfflinePolicyCorpus,
    model: &CostThresholdV1Model,
) -> Result<OfflinePolicyRegressionManifest, OfflinePolicyReferenceError> {
    let manifest = model::create(corpus, model)?;
    validate::validate(&manifest, corpus, model)?;
    Ok(manifest)
}

pub(super) fn decode(
    encoded: &[u8],
    corpus: &ValidatedOfflinePolicyCorpus,
    model: &CostThresholdV1Model,
) -> Result<OfflinePolicyRegressionManifest, OfflinePolicyReferenceError> {
    let manifest = codec::decode(encoded)?;
    validate::validate(&manifest, corpus, model)?;
    Ok(manifest)
}
