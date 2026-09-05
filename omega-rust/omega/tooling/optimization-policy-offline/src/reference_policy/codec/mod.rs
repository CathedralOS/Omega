//! Optimizer module role: stage group. Strict model and report codecs.

pub(super) mod cursor;
mod model;
mod report;

pub(super) use cursor::Cursor;

use super::model::{
    OfflinePolicyConfusion, OfflinePolicyEvaluationSummary, OfflinePolicyReferenceError,
};

pub(super) use model::{decode as decode_model, encode as encode_model};
pub(super) use report::{decode as decode_report, encode as encode_report};

pub(super) fn encode_summary(encoded: &mut Vec<u8>, summary: OfflinePolicyEvaluationSummary) {
    encoded.extend_from_slice(&summary.decision_count.to_le_bytes());
    encoded.extend_from_slice(&summary.recorded_choose_count.to_le_bytes());
    encoded.extend_from_slice(&summary.recorded_skip_count.to_le_bytes());
    encoded.extend_from_slice(&summary.predicted_choose_count.to_le_bytes());
    encoded.extend_from_slice(&summary.predicted_skip_count.to_le_bytes());
    encoded.extend_from_slice(&summary.exact_action_match_count.to_le_bytes());
    encoded.extend_from_slice(&summary.chosen_candidate_mismatch_count.to_le_bytes());
    encoded.extend_from_slice(&summary.confusion.true_choose.to_le_bytes());
    encoded.extend_from_slice(&summary.confusion.false_choose.to_le_bytes());
    encoded.extend_from_slice(&summary.confusion.true_skip.to_le_bytes());
    encoded.extend_from_slice(&summary.confusion.false_skip.to_le_bytes());
    encoded.extend_from_slice(&summary.selected_predicted_cost_delta.to_le_bytes());
}

pub(super) fn decode_summary(
    cursor: &mut Cursor<'_>,
) -> Result<OfflinePolicyEvaluationSummary, OfflinePolicyReferenceError> {
    Ok(OfflinePolicyEvaluationSummary {
        decision_count: u32::from_le_bytes(cursor.array()?),
        recorded_choose_count: u32::from_le_bytes(cursor.array()?),
        recorded_skip_count: u32::from_le_bytes(cursor.array()?),
        predicted_choose_count: u32::from_le_bytes(cursor.array()?),
        predicted_skip_count: u32::from_le_bytes(cursor.array()?),
        exact_action_match_count: u32::from_le_bytes(cursor.array()?),
        chosen_candidate_mismatch_count: u32::from_le_bytes(cursor.array()?),
        confusion: OfflinePolicyConfusion {
            true_choose: u32::from_le_bytes(cursor.array()?),
            false_choose: u32::from_le_bytes(cursor.array()?),
            true_skip: u32::from_le_bytes(cursor.array()?),
            false_skip: u32::from_le_bytes(cursor.array()?),
        },
        selected_predicted_cost_delta: i128::from_le_bytes(cursor.array()?),
    })
}
