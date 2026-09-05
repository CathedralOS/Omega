use optimization_core::OptimizationReasonCode;
use optimization_core::{ExternalDecisionAction, ExternalDecisionPoint};

use crate::{OfflinePolicyDecisionExample, OfflinePolicySplit, ValidatedOfflinePolicyCorpus};

use super::super::{
    identity::{
        cost_threshold_v1_algorithm_identity, model_identity, offline_policy_split_identity,
    },
    model::{
        CostThresholdV1Model, OfflinePolicyConfusion, OfflinePolicyEvaluationSummary,
        OfflinePolicyReferenceError,
    },
};

pub(super) fn validate(
    model: &CostThresholdV1Model,
    corpus: &ValidatedOfflinePolicyCorpus,
) -> Result<(), OfflinePolicyReferenceError> {
    if model.corpus != corpus.identity() {
        return Err(OfflinePolicyReferenceError::WrongCorpus);
    }
    if model.algorithm != cost_threshold_v1_algorithm_identity() {
        return Err(OfflinePolicyReferenceError::WrongAlgorithm);
    }
    if model.training_split != offline_policy_split_identity(corpus, OfflinePolicySplit::Training) {
        return Err(OfflinePolicyReferenceError::WrongTrainingSplit);
    }
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
    let (threshold, training) = canonical_training(&examples)?;
    if model.threshold != threshold || model.training != training {
        return Err(OfflinePolicyReferenceError::ModelMismatch);
    }
    if model.identity != model_identity(model) {
        return Err(OfflinePolicyReferenceError::ModelIdentityMismatch);
    }
    Ok(())
}

fn canonical_training(
    examples: &[&OfflinePolicyDecisionExample],
) -> Result<(i128, OfflinePolicyEvaluationSummary), OfflinePolicyReferenceError> {
    let mut thresholds = vec![i128::from(i64::MIN)];
    for example in examples {
        for candidate in example.point().legal_candidates() {
            thresholds.push(i128::from(candidate.predicted_cost_delta()) + 1);
        }
    }
    thresholds.sort_unstable();
    thresholds.dedup();
    let mut best = None;
    for threshold in thresholds {
        let summary = replay_summary(examples, threshold)?;
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
    let (_, threshold, summary) = best.expect("nonempty replay threshold roster");
    Ok((threshold, summary))
}

pub(crate) fn replay_prediction(
    point: &ExternalDecisionPoint,
    threshold: i128,
) -> (ExternalDecisionAction, Option<i64>) {
    let mut selected = None;
    for candidate in point.legal_candidates() {
        let cost = candidate.predicted_cost_delta();
        if i128::from(cost) >= threshold {
            continue;
        }
        let key = (cost, candidate.candidate());
        if selected.is_none_or(|(prior, _)| key < prior) {
            selected = Some((key, candidate));
        }
    }
    match selected {
        Some((_, candidate)) => (
            ExternalDecisionAction::Choose(candidate.candidate()),
            Some(candidate.predicted_cost_delta()),
        ),
        None => (
            ExternalDecisionAction::Skip(OptimizationReasonCode::NotProfitable),
            None,
        ),
    }
}

pub(crate) fn replay_summary(
    examples: &[&OfflinePolicyDecisionExample],
    threshold: i128,
) -> Result<OfflinePolicyEvaluationSummary, OfflinePolicyReferenceError> {
    let mut decision_count = 0_u32;
    let mut recorded_choose_count = 0_u32;
    let mut recorded_skip_count = 0_u32;
    let mut predicted_choose_count = 0_u32;
    let mut predicted_skip_count = 0_u32;
    let mut exact_action_match_count = 0_u32;
    let mut chosen_candidate_mismatch_count = 0_u32;
    let mut true_choose = 0_u32;
    let mut false_choose = 0_u32;
    let mut true_skip = 0_u32;
    let mut false_skip = 0_u32;
    let mut selected_predicted_cost_delta = 0_i128;
    for example in examples {
        increment(&mut decision_count)?;
        let recorded = example.recorded_action();
        let (predicted, cost) = replay_prediction(example.point(), threshold);
        match recorded {
            ExternalDecisionAction::Choose(_) => increment(&mut recorded_choose_count)?,
            ExternalDecisionAction::Skip(_) => increment(&mut recorded_skip_count)?,
        }
        match predicted {
            ExternalDecisionAction::Choose(_) => {
                increment(&mut predicted_choose_count)?;
                selected_predicted_cost_delta = selected_predicted_cost_delta
                    .checked_add(i128::from(
                        cost.ok_or(OfflinePolicyReferenceError::IllegalAction)?,
                    ))
                    .ok_or(OfflinePolicyReferenceError::AggregateCostOverflow)?;
            }
            ExternalDecisionAction::Skip(_) => {
                increment(&mut predicted_skip_count)?;
                if cost.is_some() {
                    return Err(OfflinePolicyReferenceError::IllegalAction);
                }
            }
        }
        match (recorded, predicted) {
            (ExternalDecisionAction::Choose(expected), ExternalDecisionAction::Choose(actual)) => {
                increment(&mut true_choose)?;
                if expected != actual {
                    increment(&mut chosen_candidate_mismatch_count)?;
                }
            }
            (ExternalDecisionAction::Skip(_), ExternalDecisionAction::Choose(_)) => {
                increment(&mut false_choose)?
            }
            (ExternalDecisionAction::Skip(_), ExternalDecisionAction::Skip(_)) => {
                increment(&mut true_skip)?
            }
            (ExternalDecisionAction::Choose(_), ExternalDecisionAction::Skip(_)) => {
                increment(&mut false_skip)?
            }
        }
        if recorded == predicted {
            increment(&mut exact_action_match_count)?;
        }
    }
    Ok(OfflinePolicyEvaluationSummary {
        decision_count,
        recorded_choose_count,
        recorded_skip_count,
        predicted_choose_count,
        predicted_skip_count,
        exact_action_match_count,
        chosen_candidate_mismatch_count,
        confusion: OfflinePolicyConfusion {
            true_choose,
            false_choose,
            true_skip,
            false_skip,
        },
        selected_predicted_cost_delta,
    })
}

fn increment(value: &mut u32) -> Result<(), OfflinePolicyReferenceError> {
    *value = value
        .checked_add(1)
        .ok_or(OfflinePolicyReferenceError::CountOverflow)?;
    Ok(())
}
