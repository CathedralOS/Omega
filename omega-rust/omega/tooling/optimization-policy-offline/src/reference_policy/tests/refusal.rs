use super::fixture::{corpus, corpus_with_prefix, corpus_without};
use crate::{
    OfflinePolicyReferenceError, OfflinePolicySplit, decode_cost_threshold_v1_model,
    decode_cost_threshold_v1_report, evaluate_cost_threshold_v1, train_cost_threshold_v1,
};

#[test]
fn empty_training_evaluation_and_regression_splits_fail_closed() {
    assert_eq!(
        train_cost_threshold_v1(&corpus_without(OfflinePolicySplit::Training)),
        Err(OfflinePolicyReferenceError::EmptySplit(
            OfflinePolicySplit::Training
        ))
    );
    let training_only = corpus_without(OfflinePolicySplit::Evaluation);
    let model = train_cost_threshold_v1(&training_only).unwrap();
    assert_eq!(
        evaluate_cost_threshold_v1(&training_only, &model, OfflinePolicySplit::Evaluation),
        Err(OfflinePolicyReferenceError::EmptySplit(
            OfflinePolicySplit::Evaluation
        ))
    );
    assert_eq!(
        evaluate_cost_threshold_v1(&training_only, &model, OfflinePolicySplit::Regression),
        Err(OfflinePolicyReferenceError::EmptySplit(
            OfflinePolicySplit::Regression
        ))
    );
}

#[test]
fn training_is_not_a_supported_evaluation_report_split() {
    let corpus = corpus();
    let model = train_cost_threshold_v1(&corpus).unwrap();
    assert_eq!(
        evaluate_cost_threshold_v1(&corpus, &model, OfflinePolicySplit::Training),
        Err(OfflinePolicyReferenceError::UnsupportedReportSplit(
            OfflinePolicySplit::Training
        ))
    );
}

#[test]
fn corpus_model_and_report_substitution_fail_closed() {
    let first = corpus_with_prefix(b"substitution-first");
    let second = corpus_with_prefix(b"substitution-second");
    let first_model = train_cost_threshold_v1(&first).unwrap();
    let second_model = train_cost_threshold_v1(&second).unwrap();
    assert_eq!(
        decode_cost_threshold_v1_model(&first_model.encode(), &second),
        Err(OfflinePolicyReferenceError::WrongCorpus)
    );
    let first_report =
        evaluate_cost_threshold_v1(&first, &first_model, OfflinePolicySplit::Evaluation).unwrap();
    assert_eq!(
        decode_cost_threshold_v1_report(&first_report.encode(), &second, &second_model),
        Err(OfflinePolicyReferenceError::WrongCorpus)
    );
    assert_eq!(
        decode_cost_threshold_v1_report(&first_report.encode(), &first, &second_model),
        Err(OfflinePolicyReferenceError::WrongCorpus)
    );
}
