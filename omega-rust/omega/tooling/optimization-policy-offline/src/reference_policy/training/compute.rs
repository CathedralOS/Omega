use optimization_core::ExternalDecisionAction;

use crate::{OfflinePolicyDecisionExample, OfflinePolicySplit, ValidatedOfflinePolicyCorpus};

use super::super::{
    identity::{
        OfflinePolicyModelIdentity, cost_threshold_v1_algorithm_identity, model_identity,
        offline_policy_split_identity,
    },
    inference::predict,
    model::{
        CostThresholdV1Model, OfflinePolicyConfusion, OfflinePolicyEvaluationSummary,
        OfflinePolicyReferenceError,
    },
};

pub(super) fn compute(
    corpus: &ValidatedOfflinePolicyCorpus,
) -> Result<CostThresholdV1Model, OfflinePolicyReferenceError> {
    let examples = corpus
        .examples()
        .iter()
        .filter(|example| example.split() == OfflinePolicySplit::Training)
        .collect::<Vec<_>>();
    if examples.is_empty() {
        return Err(OfflinePolicyReferenceError::EmptySplit(
            OfflinePolicySplit::Training,
        ));
    }
    let mut best = None;
    for threshold in thresholds(&examples) {
        let summary = summarize(&examples, threshold)?;
        let score = (
            summary.decision_count - summary.exact_action_match_count,
            threshold,
        );
        if best
            .as_ref()
            .is_none_or(|(best_score, _, _)| score < *best_score)
        {
            best = Some((score, threshold, summary));
        }
    }
    let (_, threshold, training) = best.expect("nonempty threshold roster");
    let mut model = CostThresholdV1Model {
        identity: OfflinePolicyModelIdentity::from_bytes([0; 32]),
        corpus: corpus.identity(),
        algorithm: cost_threshold_v1_algorithm_identity(),
        training_split: offline_policy_split_identity(corpus, OfflinePolicySplit::Training),
        threshold,
        training,
    };
    model.identity = model_identity(&model);
    Ok(model)
}

fn thresholds(examples: &[&OfflinePolicyDecisionExample]) -> Vec<i128> {
    let mut thresholds = vec![i128::from(i64::MIN)];
    for example in examples {
        for candidate in example.point().legal_candidates() {
            thresholds.push(i128::from(candidate.predicted_cost_delta()) + 1);
        }
    }
    thresholds.sort_unstable();
    thresholds.dedup();
    thresholds
}

fn summarize(
    examples: &[&OfflinePolicyDecisionExample],
    threshold: i128,
) -> Result<OfflinePolicyEvaluationSummary, OfflinePolicyReferenceError> {
    let mut summary = OfflinePolicyEvaluationSummary {
        decision_count: 0,
        recorded_choose_count: 0,
        recorded_skip_count: 0,
        predicted_choose_count: 0,
        predicted_skip_count: 0,
        exact_action_match_count: 0,
        chosen_candidate_mismatch_count: 0,
        confusion: OfflinePolicyConfusion {
            true_choose: 0,
            false_choose: 0,
            true_skip: 0,
            false_skip: 0,
        },
        selected_predicted_cost_delta: 0,
    };
    for example in examples {
        let recorded = example.recorded_action();
        let (predicted, selected_cost) = predict(example.point(), threshold);
        add_example(&mut summary, recorded, predicted, selected_cost)?;
    }
    Ok(summary)
}

fn add_example(
    summary: &mut OfflinePolicyEvaluationSummary,
    recorded: ExternalDecisionAction,
    predicted: ExternalDecisionAction,
    selected_cost: Option<i64>,
) -> Result<(), OfflinePolicyReferenceError> {
    checked_increment(&mut summary.decision_count)?;
    match recorded {
        ExternalDecisionAction::Choose(_) => checked_increment(&mut summary.recorded_choose_count)?,
        ExternalDecisionAction::Skip(_) => checked_increment(&mut summary.recorded_skip_count)?,
    }
    match predicted {
        ExternalDecisionAction::Choose(_) => {
            checked_increment(&mut summary.predicted_choose_count)?;
            summary.selected_predicted_cost_delta = summary
                .selected_predicted_cost_delta
                .checked_add(i128::from(
                    selected_cost.ok_or(OfflinePolicyReferenceError::IllegalAction)?,
                ))
                .ok_or(OfflinePolicyReferenceError::AggregateCostOverflow)?;
        }
        ExternalDecisionAction::Skip(_) => {
            checked_increment(&mut summary.predicted_skip_count)?;
            if selected_cost.is_some() {
                return Err(OfflinePolicyReferenceError::IllegalAction);
            }
        }
    }
    match (recorded, predicted) {
        (ExternalDecisionAction::Choose(expected), ExternalDecisionAction::Choose(actual)) => {
            checked_increment(&mut summary.confusion.true_choose)?;
            if expected != actual {
                checked_increment(&mut summary.chosen_candidate_mismatch_count)?;
            }
        }
        (ExternalDecisionAction::Skip(_), ExternalDecisionAction::Choose(_)) => {
            checked_increment(&mut summary.confusion.false_choose)?;
        }
        (ExternalDecisionAction::Skip(_), ExternalDecisionAction::Skip(_)) => {
            checked_increment(&mut summary.confusion.true_skip)?;
        }
        (ExternalDecisionAction::Choose(_), ExternalDecisionAction::Skip(_)) => {
            checked_increment(&mut summary.confusion.false_skip)?;
        }
    }
    if recorded == predicted {
        checked_increment(&mut summary.exact_action_match_count)?;
    }
    Ok(())
}

fn checked_increment(value: &mut u32) -> Result<(), OfflinePolicyReferenceError> {
    *value = value
        .checked_add(1)
        .ok_or(OfflinePolicyReferenceError::CountOverflow)?;
    Ok(())
}
