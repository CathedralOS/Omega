use super::fixture::{corpus, tie_training_corpus};
use crate::{
    OfflinePolicySplit, cost_threshold_v1_algorithm_identity, offline_policy_split_identity,
    train_cost_threshold_v1,
};

#[test]
fn training_selects_the_exact_canonical_threshold_and_binds_custody() {
    let corpus = corpus();
    let model = train_cost_threshold_v1(&corpus).unwrap();
    assert_eq!(model.corpus(), corpus.identity());
    assert_eq!(model.algorithm(), cost_threshold_v1_algorithm_identity());
    assert_eq!(
        model.training_split(),
        offline_policy_split_identity(&corpus, OfflinePolicySplit::Training)
    );
    assert_eq!(model.threshold(), 0);
    let summary = model.training_summary();
    assert_eq!(summary.decision_count(), 3);
    assert_eq!(summary.recorded_choose_count(), 2);
    assert_eq!(summary.recorded_skip_count(), 1);
    assert_eq!(summary.predicted_choose_count(), 2);
    assert_eq!(summary.predicted_skip_count(), 1);
    assert_eq!(summary.exact_action_match_count(), 3);
    assert_eq!(summary.chosen_candidate_mismatch_count(), 0);
    assert_eq!(summary.confusion().true_choose(), 2);
    assert_eq!(summary.confusion().false_choose(), 0);
    assert_eq!(summary.confusion().true_skip(), 1);
    assert_eq!(summary.confusion().false_skip(), 0);
    assert_eq!(summary.selected_predicted_cost_delta(), -4);
}

#[test]
fn equal_training_scores_choose_the_lowest_behavior_boundary() {
    let model = train_cost_threshold_v1(&tie_training_corpus()).unwrap();
    assert_eq!(model.threshold(), i128::from(i64::MIN));
    assert_eq!(model.training_summary().exact_action_match_count(), 1);
    assert_eq!(model.training_summary().predicted_choose_count(), 0);
}

#[test]
fn training_is_byte_deterministic() {
    let corpus = corpus();
    assert_eq!(
        train_cost_threshold_v1(&corpus).unwrap().encode(),
        train_cost_threshold_v1(&corpus).unwrap().encode()
    );
}
