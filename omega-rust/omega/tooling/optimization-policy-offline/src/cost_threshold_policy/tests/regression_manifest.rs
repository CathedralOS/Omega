use super::fixture::{corpus, corpus_with_prefix};
use crate::{
    OfflinePolicyReferenceError, OfflinePolicySplit, create_cost_threshold_v1_regression_manifest,
    decode_cost_threshold_v1_regression_manifest, evaluate_cost_threshold_v1,
    train_cost_threshold_v1,
};

const IDENTITY: usize = 12;
const CORPUS: usize = 44;
const MODEL: usize = 76;
const ALGORITHM: usize = 108;
const REGRESSION_SPLIT: usize = 140;
const EXPECTED_REPORT: usize = 172;
const EXPECTED_SUMMARY: usize = 204;

#[test]
fn manifest_binds_the_exact_recomputed_regression_report() {
    let corpus = corpus();
    let model = train_cost_threshold_v1(&corpus).unwrap();
    let report =
        evaluate_cost_threshold_v1(&corpus, &model, OfflinePolicySplit::Regression).unwrap();
    let manifest = create_cost_threshold_v1_regression_manifest(&corpus, &model).unwrap();
    assert_eq!(manifest.corpus(), corpus.identity());
    assert_eq!(manifest.model(), model.identity());
    assert_eq!(manifest.algorithm(), model.algorithm());
    assert_eq!(manifest.regression_split(), report.split_identity());
    assert_eq!(manifest.expected_report(), report.identity());
    assert_eq!(manifest.expected_summary(), report.summary());
    assert_eq!(
        decode_cost_threshold_v1_regression_manifest(&manifest.encode(), &corpus, &model).unwrap(),
        manifest
    );
}

#[test]
fn creation_and_checked_decode_are_byte_deterministic() {
    let corpus = corpus();
    let model = train_cost_threshold_v1(&corpus).unwrap();
    let first = create_cost_threshold_v1_regression_manifest(&corpus, &model).unwrap();
    let repeated = create_cost_threshold_v1_regression_manifest(&corpus, &model).unwrap();
    assert_eq!(first, repeated);
    assert_eq!(first.encode(), repeated.encode());
}

#[test]
fn codec_rejects_every_manifest_custody_axis_and_envelope_corruption() {
    let corpus = corpus();
    let model = train_cost_threshold_v1(&corpus).unwrap();
    let encoded = create_cost_threshold_v1_regression_manifest(&corpus, &model)
        .unwrap()
        .encode();
    assert_error(
        &corpus,
        &model,
        mutate(&encoded, 0),
        OfflinePolicyReferenceError::WrongRegressionManifestMagic,
    );
    let mut wrong_version = encoded.clone();
    wrong_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
    assert_error(
        &corpus,
        &model,
        wrong_version,
        OfflinePolicyReferenceError::UnsupportedRegressionManifestVersion(2),
    );
    for (offset, expected) in [
        (
            IDENTITY,
            OfflinePolicyReferenceError::RegressionManifestIdentityMismatch,
        ),
        (CORPUS, OfflinePolicyReferenceError::WrongCorpus),
        (MODEL, OfflinePolicyReferenceError::WrongModel),
        (ALGORITHM, OfflinePolicyReferenceError::WrongAlgorithm),
        (
            REGRESSION_SPLIT,
            OfflinePolicyReferenceError::WrongRegressionSplit,
        ),
        (
            EXPECTED_REPORT,
            OfflinePolicyReferenceError::RegressionReportMismatch,
        ),
        (
            EXPECTED_SUMMARY,
            OfflinePolicyReferenceError::RegressionSummaryMismatch,
        ),
    ] {
        assert_error(&corpus, &model, mutate(&encoded, offset), expected);
    }
    assert_error(
        &corpus,
        &model,
        encoded[..encoded.len() - 1].to_vec(),
        OfflinePolicyReferenceError::Truncated,
    );
    let mut trailing = encoded;
    trailing.push(0);
    assert_error(
        &corpus,
        &model,
        trailing,
        OfflinePolicyReferenceError::TrailingBytes,
    );
}

#[test]
fn manifest_cannot_cross_corpus_custody() {
    let first = corpus();
    let first_model = train_cost_threshold_v1(&first).unwrap();
    let encoded = create_cost_threshold_v1_regression_manifest(&first, &first_model)
        .unwrap()
        .encode();
    let second = corpus_with_prefix(b"foreign-regression-manifest");
    let second_model = train_cost_threshold_v1(&second).unwrap();
    assert_error(
        &second,
        &second_model,
        encoded,
        OfflinePolicyReferenceError::WrongCorpus,
    );
}

fn mutate(encoded: &[u8], offset: usize) -> Vec<u8> {
    let mut corrupted = encoded.to_vec();
    corrupted[offset] ^= 1;
    corrupted
}

fn assert_error(
    corpus: &crate::ValidatedOfflinePolicyCorpus,
    model: &crate::CostThresholdV1Model,
    encoded: Vec<u8>,
    expected: OfflinePolicyReferenceError,
) {
    assert_eq!(
        decode_cost_threshold_v1_regression_manifest(&encoded, corpus, model).unwrap_err(),
        expected
    );
}
