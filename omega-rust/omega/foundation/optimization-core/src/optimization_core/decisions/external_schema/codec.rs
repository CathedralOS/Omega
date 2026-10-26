use super::identity::{encode_action, encode_context};
use crate::cursor::Cursor;

use crate::{
    AnalysisSet, OptimizationCandidateIdentity, OptimizationDecisionIdentity,
    OptimizationDecisionLogIdentity, OptimizationDecisionSchemaIdentity,
    OptimizationDecisionTargetIdentity, OptimizationFactReference, OptimizationReasonCode,
    OptimizationRuleIdentity, OptimizationRuleSetIdentity, OptimizationSelectionIdentity,
    OptimizationUnitIdentity, TargetCostModelIdentity,
};

use crate::ValidatedCandidateSummary;

use super::{
    ExternalCandidateFeatures, ExternalDecisionAction, ExternalDecisionContext,
    ExternalDecisionLog, ExternalDecisionPoint, ExternalDecisionSchemaError,
};

const LOG_MAGIC: &[u8; 8] = b"OMGEXD\0\0";
const POINT_MAGIC: &[u8; 8] = b"OMGEXP\0\0";
const VERSION: u32 = 2;
const MINIMUM_CANDIDATE_WIDTH: usize = 32 + 8 + 8 + 4;

impl ExternalDecisionPoint {
    pub(super) fn encode(&self) -> Vec<u8> {
        let mut encoded = Vec::new();
        encoded.extend_from_slice(POINT_MAGIC);
        crate::codec_bytes::push_encoded(&mut encoded, VERSION.to_le_bytes());
        encoded.extend_from_slice(&self.identity.bytes());
        encoded.extend_from_slice(&self.input.bytes());
        encoded.extend_from_slice(&self.rule.bytes());
        crate::codec_bytes::push_count_u32(&mut encoded, self.legal_candidates.len());
        for candidate in &self.legal_candidates {
            encode_candidate_features(&mut encoded, candidate);
        }
        encode_action(&mut encoded, self.action);
        encoded
    }

    pub(super) fn decode(encoded: &[u8]) -> Result<Self, ExternalDecisionSchemaError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != POINT_MAGIC {
            return Err(ExternalDecisionSchemaError::WrongPointMagic);
        }
        let version = cursor.u32()?;
        if version != VERSION {
            return Err(ExternalDecisionSchemaError::UnsupportedPointVersion(
                version,
            ));
        }
        let claimed = OptimizationDecisionIdentity::from_bytes(cursor.array()?);
        let input = OptimizationUnitIdentity::from_bytes(cursor.array()?);
        let rule = OptimizationRuleIdentity::from_bytes(cursor.array()?);
        let count = cursor.u32()? as usize;
        if count > cursor.remaining().saturating_sub(2) / MINIMUM_CANDIDATE_WIDTH {
            return Err(ExternalDecisionSchemaError::Truncated);
        }
        let mut candidates = Vec::with_capacity(count);
        for _ in 0..count {
            candidates.push(decode_candidate_features(&mut cursor)?);
        }
        if candidates
            .windows(2)
            .any(|pair| pair[0].candidate() >= pair[1].candidate())
        {
            return Err(ExternalDecisionSchemaError::NonCanonicalCandidates);
        }
        let action = decode_action(&mut cursor)?;
        if cursor.remaining() != 0 {
            return Err(ExternalDecisionSchemaError::TrailingBytes);
        }
        let point = Self::new(input, rule, candidates, action)?;
        if point.identity != claimed {
            return Err(ExternalDecisionSchemaError::PointIdentityMismatch);
        }
        Ok(point)
    }
}

impl ExternalDecisionLog {
    pub fn encode(&self) -> Vec<u8> {
        let mut encoded = Vec::new();
        encoded.extend_from_slice(LOG_MAGIC);
        crate::codec_bytes::push_encoded(&mut encoded, VERSION.to_le_bytes());
        encoded.extend_from_slice(&self.identity.bytes());
        encode_context(&mut encoded, self.context);
        crate::codec_bytes::push_count_u32(&mut encoded, self.points.len());
        for point in &self.points {
            let point = point.encode();
            crate::codec_bytes::push_count_u32(&mut encoded, point.len());
            encoded.extend_from_slice(&point);
        }
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, ExternalDecisionSchemaError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != LOG_MAGIC {
            return Err(ExternalDecisionSchemaError::WrongLogMagic);
        }
        let version = cursor.u32()?;
        if version != VERSION {
            return Err(ExternalDecisionSchemaError::UnsupportedLogVersion(version));
        }
        let claimed = OptimizationDecisionLogIdentity::from_bytes(cursor.array()?);
        let context = decode_context(&mut cursor)?;
        let count = cursor.u32()? as usize;
        if count > cursor.remaining() / 4 {
            return Err(ExternalDecisionSchemaError::Truncated);
        }
        let mut points = Vec::with_capacity(count);
        for _ in 0..count {
            let length = cursor.u32()? as usize;
            points.push(ExternalDecisionPoint::decode(cursor.take(length)?)?);
        }
        if cursor.remaining() != 0 {
            return Err(ExternalDecisionSchemaError::TrailingBytes);
        }
        let log = Self::new(context, points)?;
        if log.identity != claimed {
            return Err(ExternalDecisionSchemaError::LogIdentityMismatch);
        }
        Ok(log)
    }
}

fn encode_candidate_features(encoded: &mut Vec<u8>, features: &ExternalCandidateFeatures) {
    encoded.extend_from_slice(&features.summary.candidate.bytes());
    crate::codec_bytes::push_encoded(encoded, features.summary.predicted_cost_delta.to_le_bytes());
    encoded.extend_from_slice(&features.consumed_analyses.encode());
    crate::codec_bytes::push_count_u32(encoded, features.consumed_facts.len());
    for fact in &features.consumed_facts {
        encoded.extend_from_slice(&fact.encode());
    }
}

fn decode_candidate_features(
    cursor: &mut Cursor<'_>,
) -> Result<ExternalCandidateFeatures, ExternalDecisionSchemaError> {
    let summary = ValidatedCandidateSummary {
        candidate: OptimizationCandidateIdentity::from_bytes(cursor.array()?),
        predicted_cost_delta: cursor.i64()?,
    };
    let consumed_analyses = AnalysisSet::decode(cursor.take(8)?)
        .map_err(ExternalDecisionSchemaError::InvalidAnalysisSet)?;
    let fact_count = cursor.u32()? as usize;
    if fact_count > cursor.remaining() / OptimizationFactReference::ENCODED_LENGTH {
        return Err(ExternalDecisionSchemaError::Truncated);
    }
    let mut consumed_facts = Vec::with_capacity(fact_count);
    for _ in 0..fact_count {
        consumed_facts.push(
            OptimizationFactReference::decode(
                cursor.take(OptimizationFactReference::ENCODED_LENGTH)?,
            )
            .map_err(ExternalDecisionSchemaError::InvalidFactReference)?,
        );
    }
    if consumed_facts.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ExternalDecisionSchemaError::NonCanonicalCandidateFacts);
    }
    ExternalCandidateFeatures::new(summary, consumed_analyses, consumed_facts)
}

fn decode_context(
    cursor: &mut Cursor<'_>,
) -> Result<ExternalDecisionContext, ExternalDecisionSchemaError> {
    Ok(ExternalDecisionContext::new(
        OptimizationDecisionSchemaIdentity::from_bytes(cursor.array()?),
        OptimizationUnitIdentity::from_bytes(cursor.array()?),
        OptimizationSelectionIdentity::from_bytes(cursor.array()?),
        OptimizationSelectionIdentity::from_bytes(cursor.array()?),
        OptimizationDecisionTargetIdentity::from_bytes(cursor.array()?),
        OptimizationRuleSetIdentity::from_bytes(cursor.array()?),
        TargetCostModelIdentity::from_bytes(cursor.array()?),
    ))
}

fn decode_action(
    cursor: &mut Cursor<'_>,
) -> Result<ExternalDecisionAction, ExternalDecisionSchemaError> {
    match cursor.byte()? {
        1 => Ok(ExternalDecisionAction::Choose(
            OptimizationCandidateIdentity::from_bytes(cursor.array()?),
        )),
        2 => match cursor.byte()? {
            reason if reason == OptimizationReasonCode::NotProfitable as u8 => Ok(
                ExternalDecisionAction::Skip(OptimizationReasonCode::NotProfitable),
            ),
            reason => Err(ExternalDecisionSchemaError::UnknownReason(reason)),
        },
        action => Err(ExternalDecisionSchemaError::UnknownAction(action)),
    }
}

crate::truncated_from_adapter!(ExternalDecisionSchemaError);
