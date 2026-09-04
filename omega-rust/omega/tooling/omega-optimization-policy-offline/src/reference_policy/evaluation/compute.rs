use omega_optimization_policy::ExternalDecisionAction;

use crate::{OfflinePolicyDecisionExample, OfflinePolicySplit, ValidatedOfflinePolicyCorpus};

use super::super::{
    identity::{OfflinePolicyReportIdentity, offline_policy_split_identity, report_identity},
    inference::predict,
    model::{
        CostThresholdV1Model, OfflinePolicyConfusion, OfflinePolicyEvaluationReport,
        OfflinePolicyEvaluationSummary, OfflinePolicyPrediction, OfflinePolicyReferenceError,
    },
};

pub(super) fn compute(
    corpus: &ValidatedOfflinePolicyCorpus,
    model: &CostThresholdV1Model,
    split: OfflinePolicySplit,
) -> Result<OfflinePolicyEvaluationReport, OfflinePolicyReferenceError> {
    let examples = corpus
        .examples()
        .iter()
        .filter(|example| example.split() == split)
        .collect::<Vec<_>>();
    if examples.is_empty() {
        return Err(OfflinePolicyReferenceError::EmptySplit(split));
    }
    let predictions = examples
        .iter()
        .map(|example| {
            let (action, selected_predicted_cost_delta) = predict(example.point(), model.threshold);
            OfflinePolicyPrediction {
                surface: example.surface(),
                action,
                selected_predicted_cost_delta,
            }
        })
        .collect::<Vec<_>>();
    let summary = summarize(&examples, &predictions)?;
    let mut report = OfflinePolicyEvaluationReport {
        identity: OfflinePolicyReportIdentity::from_bytes([0; 32]),
        corpus: corpus.identity(),
        model: model.identity,
        algorithm: model.algorithm,
        split,
        split_identity: offline_policy_split_identity(corpus, split),
        predictions,
        summary,
    };
    report.identity = report_identity(&report);
    Ok(report)
}

fn summarize(
    examples: &[&OfflinePolicyDecisionExample],
    predictions: &[OfflinePolicyPrediction],
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
    for (example, prediction) in examples.iter().zip(predictions) {
        add(
            &mut summary,
            example.recorded_action(),
            prediction.action,
            prediction.selected_predicted_cost_delta,
        )?;
    }
    Ok(summary)
}

fn add(
    summary: &mut OfflinePolicyEvaluationSummary,
    recorded: ExternalDecisionAction,
    predicted: ExternalDecisionAction,
    cost: Option<i64>,
) -> Result<(), OfflinePolicyReferenceError> {
    increment(&mut summary.decision_count)?;
    match recorded {
        ExternalDecisionAction::Choose(_) => increment(&mut summary.recorded_choose_count)?,
        ExternalDecisionAction::Skip(_) => increment(&mut summary.recorded_skip_count)?,
    }
    match predicted {
        ExternalDecisionAction::Choose(_) => {
            increment(&mut summary.predicted_choose_count)?;
            summary.selected_predicted_cost_delta = summary
                .selected_predicted_cost_delta
                .checked_add(i128::from(
                    cost.ok_or(OfflinePolicyReferenceError::IllegalAction)?,
                ))
                .ok_or(OfflinePolicyReferenceError::AggregateCostOverflow)?;
        }
        ExternalDecisionAction::Skip(_) => {
            increment(&mut summary.predicted_skip_count)?;
            if cost.is_some() {
                return Err(OfflinePolicyReferenceError::IllegalAction);
            }
        }
    }
    match (recorded, predicted) {
        (ExternalDecisionAction::Choose(expected), ExternalDecisionAction::Choose(actual)) => {
            increment(&mut summary.confusion.true_choose)?;
            if expected != actual {
                increment(&mut summary.chosen_candidate_mismatch_count)?;
            }
        }
        (ExternalDecisionAction::Skip(_), ExternalDecisionAction::Choose(_)) => {
            increment(&mut summary.confusion.false_choose)?
        }
        (ExternalDecisionAction::Skip(_), ExternalDecisionAction::Skip(_)) => {
            increment(&mut summary.confusion.true_skip)?
        }
        (ExternalDecisionAction::Choose(_), ExternalDecisionAction::Skip(_)) => {
            increment(&mut summary.confusion.false_skip)?
        }
    }
    if recorded == predicted {
        increment(&mut summary.exact_action_match_count)?;
    }
    Ok(())
}

fn increment(value: &mut u32) -> Result<(), OfflinePolicyReferenceError> {
    *value = value
        .checked_add(1)
        .ok_or(OfflinePolicyReferenceError::CountOverflow)?;
    Ok(())
}
