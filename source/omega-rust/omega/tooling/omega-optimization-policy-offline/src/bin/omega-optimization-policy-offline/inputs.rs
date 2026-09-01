//! Strict corpus and model artifact admission for offline commands.

use std::{fs, path::Path};

use omega_optimization_policy_offline::{
    CostThresholdV1Model, ValidatedOfflinePolicyCorpus, decode_cost_threshold_v1_model,
    decode_offline_policy_corpus,
};

use crate::error::OfflinePolicyCommandError;

pub(super) fn read_corpus(
    path: &Path,
) -> Result<ValidatedOfflinePolicyCorpus, OfflinePolicyCommandError> {
    let encoded = fs::read(path).map_err(|source| OfflinePolicyCommandError::ReadCorpus {
        path: path.to_path_buf(),
        source,
    })?;
    decode_offline_policy_corpus(&encoded).map_err(OfflinePolicyCommandError::InvalidCorpus)
}

pub(super) fn read_model(
    path: &Path,
    corpus: &ValidatedOfflinePolicyCorpus,
) -> Result<CostThresholdV1Model, OfflinePolicyCommandError> {
    let encoded = fs::read(path).map_err(|source| OfflinePolicyCommandError::ReadModel {
        path: path.to_path_buf(),
        source,
    })?;
    decode_cost_threshold_v1_model(&encoded, corpus)
        .map_err(OfflinePolicyCommandError::InvalidReferenceArtifact)
}
