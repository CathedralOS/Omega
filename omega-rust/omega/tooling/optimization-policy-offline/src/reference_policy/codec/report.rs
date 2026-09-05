use optimization_core::ExternalDecisionAction;
use optimization_core::{OptimizationCandidateIdentity, OptimizationReasonCode};

use crate::{
    DecisionSurfaceIdentity, OfflinePolicyCorpusIdentity, OfflinePolicySplit,
    ValidatedOfflinePolicyCorpus,
};

use super::super::{
    evaluation,
    identity::{
        OfflinePolicyAlgorithmIdentity, OfflinePolicyModelIdentity, OfflinePolicyReportIdentity,
        OfflinePolicySplitIdentity,
    },
    model::{
        CostThresholdV1Model, OfflinePolicyEvaluationReport, OfflinePolicyPrediction,
        OfflinePolicyReferenceError,
    },
};
use super::{Cursor, decode_summary, encode_summary};

const MAGIC: &[u8; 8] = b"OMGOPR\0\0";
const VERSION: u32 = 1;
const MINIMUM_PREDICTION_LENGTH: usize = 32 + 1 + 1;

pub(crate) fn encode(report: &OfflinePolicyEvaluationReport) -> Vec<u8> {
    let mut encoded = Vec::new();
    encoded.extend_from_slice(MAGIC);
    encoded.extend_from_slice(&VERSION.to_le_bytes());
    encoded.extend_from_slice(&report.identity.bytes());
    encoded.extend_from_slice(&report.corpus.bytes());
    encoded.extend_from_slice(&report.model.bytes());
    encoded.extend_from_slice(&report.algorithm.bytes());
    encoded.push(report.split.tag());
    encoded.extend_from_slice(&report.split_identity.bytes());
    encoded.extend_from_slice(
        &u32::try_from(report.predictions.len())
            .expect("validated report prediction count fits u32")
            .to_le_bytes(),
    );
    for prediction in &report.predictions {
        encode_prediction(&mut encoded, *prediction);
    }
    encode_summary(&mut encoded, report.summary);
    encoded
}

pub(crate) fn decode(
    encoded: &[u8],
    corpus: &ValidatedOfflinePolicyCorpus,
    model: &CostThresholdV1Model,
) -> Result<OfflinePolicyEvaluationReport, OfflinePolicyReferenceError> {
    let mut cursor = Cursor::new(encoded);
    if cursor.take(8)? != MAGIC {
        return Err(OfflinePolicyReferenceError::WrongReportMagic);
    }
    let version = u32::from_le_bytes(cursor.array()?);
    if version != VERSION {
        return Err(OfflinePolicyReferenceError::UnsupportedReportVersion(
            version,
        ));
    }
    let identity = OfflinePolicyReportIdentity::from_bytes(cursor.array()?);
    let corpus_identity = OfflinePolicyCorpusIdentity::from_bytes(cursor.array()?);
    let model_identity = OfflinePolicyModelIdentity::from_bytes(cursor.array()?);
    let algorithm = OfflinePolicyAlgorithmIdentity::from_bytes(cursor.array()?);
    let split = decode_split(cursor.byte()?)?;
    let split_identity = OfflinePolicySplitIdentity::from_bytes(cursor.array()?);
    let count = u32::from_le_bytes(cursor.array()?) as usize;
    if count > cursor.remaining().saturating_sub(60) / MINIMUM_PREDICTION_LENGTH {
        return Err(OfflinePolicyReferenceError::Truncated);
    }
    let mut predictions = Vec::with_capacity(count);
    for _ in 0..count {
        predictions.push(decode_prediction(&mut cursor)?);
    }
    let summary = decode_summary(&mut cursor)?;
    if cursor.remaining() != 0 {
        return Err(OfflinePolicyReferenceError::TrailingBytes);
    }
    let report = OfflinePolicyEvaluationReport {
        identity,
        corpus: corpus_identity,
        model: model_identity,
        algorithm,
        split,
        split_identity,
        predictions,
        summary,
    };
    evaluation::validate(&report, corpus, model)?;
    Ok(report)
}

fn encode_prediction(encoded: &mut Vec<u8>, prediction: OfflinePolicyPrediction) {
    encoded.extend_from_slice(&prediction.surface.bytes());
    match prediction.action {
        ExternalDecisionAction::Choose(candidate) => {
            encoded.push(1);
            encoded.extend_from_slice(&candidate.bytes());
        }
        ExternalDecisionAction::Skip(reason) => {
            encoded.push(2);
            encoded.push(reason as u8);
        }
    }
    match prediction.selected_predicted_cost_delta {
        Some(cost) => {
            encoded.push(1);
            encoded.extend_from_slice(&cost.to_le_bytes());
        }
        None => encoded.push(0),
    }
}

fn decode_prediction(
    cursor: &mut Cursor<'_>,
) -> Result<OfflinePolicyPrediction, OfflinePolicyReferenceError> {
    let surface = DecisionSurfaceIdentity::from_bytes(cursor.array()?);
    let action = match cursor.byte()? {
        1 => ExternalDecisionAction::Choose(OptimizationCandidateIdentity::from_bytes(
            cursor.array()?,
        )),
        2 => ExternalDecisionAction::Skip(decode_reason(cursor.byte()?)?),
        tag => return Err(OfflinePolicyReferenceError::UnknownAction(tag)),
    };
    let selected_predicted_cost_delta = match cursor.byte()? {
        0 => None,
        1 => Some(i64::from_le_bytes(cursor.array()?)),
        tag => return Err(OfflinePolicyReferenceError::UnknownAction(tag)),
    };
    match (action, selected_predicted_cost_delta) {
        (ExternalDecisionAction::Choose(_), Some(_)) | (ExternalDecisionAction::Skip(_), None) => {}
        _ => return Err(OfflinePolicyReferenceError::IllegalAction),
    }
    Ok(OfflinePolicyPrediction {
        surface,
        action,
        selected_predicted_cost_delta,
    })
}

fn decode_split(tag: u8) -> Result<OfflinePolicySplit, OfflinePolicyReferenceError> {
    match tag {
        1 => Ok(OfflinePolicySplit::Training),
        2 => Ok(OfflinePolicySplit::Evaluation),
        3 => Ok(OfflinePolicySplit::Regression),
        _ => Err(OfflinePolicyReferenceError::ReportMismatch),
    }
}

fn decode_reason(tag: u8) -> Result<OptimizationReasonCode, OfflinePolicyReferenceError> {
    OptimizationReasonCode::ALL
        .into_iter()
        .find(|reason| *reason as u8 == tag)
        .ok_or(OfflinePolicyReferenceError::UnknownReason(tag))
}
