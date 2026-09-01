//! Explicit creation and read-only checking of regression baselines.

use std::fs;

use omega_optimization_policy_offline::{
    create_cost_threshold_v1_regression_manifest, decode_cost_threshold_v1_regression_manifest,
};

use crate::{
    arguments::{RegressionManifestCheckRequest, RegressionManifestCreationRequest},
    error::OfflinePolicyCommandError,
    inputs::{read_corpus, read_model},
    publication::publish_new,
};

pub(super) fn create(
    request: RegressionManifestCreationRequest,
) -> Result<(), OfflinePolicyCommandError> {
    let corpus = read_corpus(&request.corpus)?;
    let model = read_model(&request.model, &corpus)?;
    let manifest = create_cost_threshold_v1_regression_manifest(&corpus, &model)
        .map_err(OfflinePolicyCommandError::InvalidReferenceArtifact)?;
    publish_new(&request.output, &manifest.encode())
}

pub(super) fn check(
    request: RegressionManifestCheckRequest,
) -> Result<(), OfflinePolicyCommandError> {
    let corpus = read_corpus(&request.corpus)?;
    let model = read_model(&request.model, &corpus)?;
    let encoded =
        fs::read(&request.manifest).map_err(|source| OfflinePolicyCommandError::ReadManifest {
            path: request.manifest,
            source,
        })?;
    decode_cost_threshold_v1_regression_manifest(&encoded, &corpus, &model)
        .map_err(OfflinePolicyCommandError::InvalidReferenceArtifact)?;
    Ok(())
}
