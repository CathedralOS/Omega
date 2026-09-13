use super::fixture::corpus;
use crate::{
    OfflinePolicyReferenceError, OfflinePolicySplit, decode_cost_threshold_v1_model,
    decode_cost_threshold_v1_report, evaluate_cost_threshold_v1, train_cost_threshold_v1,
};

const MODEL_IDENTITY: usize = 12;
const MODEL_CORPUS: usize = 44;
const MODEL_ALGORITHM: usize = 76;
const MODEL_TRAINING_SPLIT: usize = 108;
const MODEL_THRESHOLD: usize = 140;
const MODEL_SUMMARY: usize = 156;

const REPORT_IDENTITY: usize = 12;
const REPORT_CORPUS: usize = 44;
const REPORT_MODEL: usize = 76;
const REPORT_ALGORITHM: usize = 108;
const REPORT_SPLIT: usize = 140;
const REPORT_SPLIT_IDENTITY: usize = 141;
const REPORT_FIRST_ACTION: usize = 209;
const REPORT_FIRST_CANDIDATE: usize = 210;
const REPORT_FIRST_COST_PRESENCE: usize = 242;
const REPORT_FIRST_COST: usize = 243;

#[test]
fn model_and_report_codecs_round_trip_exact_validated_artifacts() {
    let corpus = corpus();
    let model = train_cost_threshold_v1(&corpus).unwrap();
    let decoded_model = decode_cost_threshold_v1_model(&model.encode(), &corpus).unwrap();
    assert_eq!(decoded_model, model);
    let report =
        evaluate_cost_threshold_v1(&corpus, &model, OfflinePolicySplit::Evaluation).unwrap();
    let decoded_report =
        decode_cost_threshold_v1_report(&report.encode(), &corpus, &model).unwrap();
    assert_eq!(decoded_report, report);
}

#[test]
fn model_codec_rejects_every_custody_axis_and_envelope_corruption() {
    let corpus = corpus();
    let model = train_cost_threshold_v1(&corpus).unwrap();
    let encoded = model.encode();
    assert_model_error(
        &corpus,
        mutate(&encoded, 0),
        OfflinePolicyReferenceError::WrongModelMagic,
    );
    let mut wrong_version = encoded.clone();
    wrong_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
    assert_model_error(
        &corpus,
        wrong_version,
        OfflinePolicyReferenceError::UnsupportedModelVersion(2),
    );
    assert_model_error(
        &corpus,
        mutate(&encoded, MODEL_IDENTITY),
        OfflinePolicyReferenceError::ModelIdentityMismatch,
    );
    assert_model_error(
        &corpus,
        mutate(&encoded, MODEL_CORPUS),
        OfflinePolicyReferenceError::WrongCorpus,
    );
    assert_model_error(
        &corpus,
        mutate(&encoded, MODEL_ALGORITHM),
        OfflinePolicyReferenceError::WrongAlgorithm,
    );
    assert_model_error(
        &corpus,
        mutate(&encoded, MODEL_TRAINING_SPLIT),
        OfflinePolicyReferenceError::WrongTrainingSplit,
    );
    assert_model_error(
        &corpus,
        mutate(&encoded, MODEL_THRESHOLD),
        OfflinePolicyReferenceError::ModelMismatch,
    );
    assert_model_error(
        &corpus,
        mutate(&encoded, MODEL_SUMMARY),
        OfflinePolicyReferenceError::ModelMismatch,
    );
    assert_model_error(
        &corpus,
        encoded[..encoded.len() - 1].to_vec(),
        OfflinePolicyReferenceError::Truncated,
    );
    let mut trailing = encoded;
    trailing.push(0);
    assert_model_error(
        &corpus,
        trailing,
        OfflinePolicyReferenceError::TrailingBytes,
    );
}

#[test]
fn report_codec_rejects_identity_split_action_cost_and_envelope_corruption() {
    let corpus = corpus();
    let model = train_cost_threshold_v1(&corpus).unwrap();
    let report =
        evaluate_cost_threshold_v1(&corpus, &model, OfflinePolicySplit::Evaluation).unwrap();
    let encoded = report.encode();
    assert_report_error(
        &corpus,
        &model,
        mutate(&encoded, 0),
        OfflinePolicyReferenceError::WrongReportMagic,
    );
    let mut wrong_version = encoded.clone();
    wrong_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
    assert_report_error(
        &corpus,
        &model,
        wrong_version,
        OfflinePolicyReferenceError::UnsupportedReportVersion(2),
    );
    assert_report_error(
        &corpus,
        &model,
        mutate(&encoded, REPORT_IDENTITY),
        OfflinePolicyReferenceError::ReportIdentityMismatch,
    );
    assert_report_error(
        &corpus,
        &model,
        mutate(&encoded, REPORT_CORPUS),
        OfflinePolicyReferenceError::WrongCorpus,
    );
    assert_report_error(
        &corpus,
        &model,
        mutate(&encoded, REPORT_MODEL),
        OfflinePolicyReferenceError::WrongModel,
    );
    assert_report_error(
        &corpus,
        &model,
        mutate(&encoded, REPORT_ALGORITHM),
        OfflinePolicyReferenceError::WrongAlgorithm,
    );
    let mut training_split = encoded.clone();
    training_split[REPORT_SPLIT] = 1;
    assert_report_error(
        &corpus,
        &model,
        training_split,
        OfflinePolicyReferenceError::UnsupportedReportSplit(OfflinePolicySplit::Training),
    );
    assert_report_error(
        &corpus,
        &model,
        mutate(&encoded, REPORT_SPLIT_IDENTITY),
        OfflinePolicyReferenceError::ReportMismatch,
    );
    let mut zero_action = encoded.clone();
    zero_action[REPORT_FIRST_ACTION] = 0;
    assert_report_error(
        &corpus,
        &model,
        zero_action,
        OfflinePolicyReferenceError::UnknownAction(0),
    );
    assert_report_error(
        &corpus,
        &model,
        mutate(&encoded, REPORT_FIRST_CANDIDATE),
        OfflinePolicyReferenceError::NonCanonicalPredictions,
    );
    let mut missing_chosen_cost = encoded.clone();
    missing_chosen_cost[REPORT_FIRST_COST_PRESENCE] = 0;
    assert_report_error(
        &corpus,
        &model,
        missing_chosen_cost,
        OfflinePolicyReferenceError::IllegalAction,
    );
    assert_report_error(
        &corpus,
        &model,
        mutate(&encoded, REPORT_FIRST_COST),
        OfflinePolicyReferenceError::NonCanonicalPredictions,
    );
    assert_report_error(
        &corpus,
        &model,
        mutate(&encoded, encoded.len() - 1),
        OfflinePolicyReferenceError::ReportMismatch,
    );
    assert_report_error(
        &corpus,
        &model,
        encoded[..encoded.len() - 1].to_vec(),
        OfflinePolicyReferenceError::Truncated,
    );
    let mut trailing = encoded;
    trailing.push(0);
    assert_report_error(
        &corpus,
        &model,
        trailing,
        OfflinePolicyReferenceError::TrailingBytes,
    );
}

fn mutate(encoded: &[u8], offset: usize) -> Vec<u8> {
    let mut corrupted = encoded.to_vec();
    corrupted[offset] ^= 1;
    corrupted
}

fn assert_model_error(
    corpus: &crate::ValidatedOfflinePolicyCorpus,
    encoded: Vec<u8>,
    expected: OfflinePolicyReferenceError,
) {
    assert_eq!(
        decode_cost_threshold_v1_model(&encoded, corpus),
        Err(expected)
    );
}

fn assert_report_error(
    corpus: &crate::ValidatedOfflinePolicyCorpus,
    model: &crate::CostThresholdV1Model,
    encoded: Vec<u8>,
    expected: OfflinePolicyReferenceError,
) {
    assert_eq!(
        decode_cost_threshold_v1_report(&encoded, corpus, model),
        Err(expected)
    );
}
