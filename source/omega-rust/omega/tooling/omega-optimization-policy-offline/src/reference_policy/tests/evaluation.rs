use omega_optimization_policy::ExternalDecisionAction;

use super::fixture::{canonical_tie_candidate, corpus, i128_aggregate_corpus};
use crate::{OfflinePolicySplit, evaluate_cost_threshold_v1, train_cost_threshold_v1};

#[test]
fn evaluation_report_pins_threshold_boundary_tie_order_and_exact_counts() {
    let corpus = corpus();
    let model = train_cost_threshold_v1(&corpus).unwrap();
    let report =
        evaluate_cost_threshold_v1(&corpus, &model, OfflinePolicySplit::Evaluation).unwrap();
    assert_eq!(report.predictions().len(), 3);
    assert_eq!(
        report.predictions()[0].action(),
        ExternalDecisionAction::Choose(canonical_tie_candidate(b"reference", b"evaluation-tie"))
    );
    assert_eq!(
        report.predictions()[0].selected_predicted_cost_delta(),
        Some(-2)
    );
    assert!(matches!(
        report.predictions()[1].action(),
        ExternalDecisionAction::Skip(_)
    ));
    assert_eq!(
        report.predictions()[1].selected_predicted_cost_delta(),
        None
    );
    let summary = report.summary();
    assert_eq!(summary.decision_count(), 3);
    assert_eq!(summary.exact_action_match_count(), 3);
    assert_eq!(summary.confusion().true_choose(), 2);
    assert_eq!(summary.confusion().true_skip(), 1);
    assert_eq!(summary.selected_predicted_cost_delta(), -3);
}

#[test]
fn regression_report_distinguishes_label_agreement_from_binary_confusion() {
    let corpus = corpus();
    let model = train_cost_threshold_v1(&corpus).unwrap();
    let report =
        evaluate_cost_threshold_v1(&corpus, &model, OfflinePolicySplit::Regression).unwrap();
    let summary = report.summary();
    assert_eq!(summary.decision_count(), 3);
    assert_eq!(summary.recorded_choose_count(), 2);
    assert_eq!(summary.recorded_skip_count(), 1);
    assert_eq!(summary.predicted_choose_count(), 2);
    assert_eq!(summary.predicted_skip_count(), 1);
    assert_eq!(summary.confusion().true_choose(), 1);
    assert_eq!(summary.confusion().false_choose(), 1);
    assert_eq!(summary.confusion().true_skip(), 0);
    assert_eq!(summary.confusion().false_skip(), 1);
    assert_eq!(summary.chosen_candidate_mismatch_count(), 1);
    assert_eq!(summary.exact_action_match_count(), 0);
    assert_eq!(summary.selected_predicted_cost_delta(), -6);
}

#[test]
fn aggregate_selected_delta_is_checked_i128_not_wrapping_i64() {
    let corpus = i128_aggregate_corpus();
    let model = train_cost_threshold_v1(&corpus).unwrap();
    assert_eq!(model.threshold(), i128::from(i64::MAX) + 1);
    let report =
        evaluate_cost_threshold_v1(&corpus, &model, OfflinePolicySplit::Evaluation).unwrap();
    assert_eq!(
        report.summary().selected_predicted_cost_delta(),
        i128::from(i64::MIN) * 2
    );
}

#[test]
fn evaluation_and_regression_reports_are_deterministic_and_distinct() {
    let corpus = corpus();
    let model = train_cost_threshold_v1(&corpus).unwrap();
    let first =
        evaluate_cost_threshold_v1(&corpus, &model, OfflinePolicySplit::Evaluation).unwrap();
    let repeated =
        evaluate_cost_threshold_v1(&corpus, &model, OfflinePolicySplit::Evaluation).unwrap();
    let regression =
        evaluate_cost_threshold_v1(&corpus, &model, OfflinePolicySplit::Regression).unwrap();
    assert_eq!(first, repeated);
    assert_eq!(first.encode(), repeated.encode());
    assert_ne!(first.identity(), regression.identity());
    assert_ne!(first.split_identity(), regression.split_identity());
}
