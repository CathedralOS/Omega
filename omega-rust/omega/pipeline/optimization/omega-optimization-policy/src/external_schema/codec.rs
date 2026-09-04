use omega_optimization_core::{
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
        encoded.extend_from_slice(&VERSION.to_le_bytes());
        encoded.extend_from_slice(&self.identity.bytes());
        encoded.extend_from_slice(&self.input.bytes());
        encoded.extend_from_slice(&self.rule.bytes());
        encoded.extend_from_slice(
            &u32::try_from(self.legal_candidates.len())
                .expect("external decision candidate count fits u32")
                .to_le_bytes(),
        );
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
        let version = u32::from_le_bytes(cursor.array()?);
        if version != VERSION {
            return Err(ExternalDecisionSchemaError::UnsupportedPointVersion(
                version,
            ));
        }
        let claimed = OptimizationDecisionIdentity::from_bytes(cursor.array()?);
        let input = OptimizationUnitIdentity::from_bytes(cursor.array()?);
        let rule = OptimizationRuleIdentity::from_bytes(cursor.array()?);
        let count = u32::from_le_bytes(cursor.array()?) as usize;
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
        encoded.extend_from_slice(&VERSION.to_le_bytes());
        encoded.extend_from_slice(&self.identity.bytes());
        encode_context(&mut encoded, self.context);
        encoded.extend_from_slice(
            &u32::try_from(self.points.len())
                .expect("external decision point count fits u32")
                .to_le_bytes(),
        );
        for point in &self.points {
            let point = point.encode();
            encoded.extend_from_slice(
                &u32::try_from(point.len())
                    .expect("external decision point encoding fits u32")
                    .to_le_bytes(),
            );
            encoded.extend_from_slice(&point);
        }
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, ExternalDecisionSchemaError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != LOG_MAGIC {
            return Err(ExternalDecisionSchemaError::WrongLogMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != VERSION {
            return Err(ExternalDecisionSchemaError::UnsupportedLogVersion(version));
        }
        let claimed = OptimizationDecisionLogIdentity::from_bytes(cursor.array()?);
        let context = decode_context(&mut cursor)?;
        let count = u32::from_le_bytes(cursor.array()?) as usize;
        if count > cursor.remaining() / 4 {
            return Err(ExternalDecisionSchemaError::Truncated);
        }
        let mut points = Vec::with_capacity(count);
        for _ in 0..count {
            let length = u32::from_le_bytes(cursor.array()?) as usize;
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
    encoded.extend_from_slice(&features.summary.predicted_cost_delta.to_le_bytes());
    encoded.extend_from_slice(&features.consumed_analyses.encode());
    encoded.extend_from_slice(
        &u32::try_from(features.consumed_facts.len())
            .expect("external candidate fact count fits u32")
            .to_le_bytes(),
    );
    for fact in &features.consumed_facts {
        encoded.extend_from_slice(&fact.encode());
    }
}

fn decode_candidate_features(
    cursor: &mut Cursor<'_>,
) -> Result<ExternalCandidateFeatures, ExternalDecisionSchemaError> {
    let summary = ValidatedCandidateSummary {
        candidate: OptimizationCandidateIdentity::from_bytes(cursor.array()?),
        predicted_cost_delta: i64::from_le_bytes(cursor.array()?),
    };
    let consumed_analyses = AnalysisSet::decode(cursor.take(8)?)
        .map_err(ExternalDecisionSchemaError::InvalidAnalysisSet)?;
    let fact_count = u32::from_le_bytes(cursor.array()?) as usize;
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

fn encode_context(encoded: &mut Vec<u8>, context: ExternalDecisionContext) {
    encoded.extend_from_slice(&context.schema.bytes());
    encoded.extend_from_slice(&context.source.bytes());
    encoded.extend_from_slice(&context.selections.bytes());
    encoded.extend_from_slice(&context.phase_selections.bytes());
    encoded.extend_from_slice(&context.target.bytes());
    encoded.extend_from_slice(&context.rule_set.bytes());
    encoded.extend_from_slice(&context.cost_model.bytes());
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

fn encode_action(encoded: &mut Vec<u8>, action: ExternalDecisionAction) {
    match action {
        ExternalDecisionAction::Choose(candidate) => {
            encoded.push(1);
            encoded.extend_from_slice(&candidate.bytes());
        }
        ExternalDecisionAction::Skip(reason) => {
            encoded.push(2);
            encoded.push(reason as u8);
        }
    }
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

struct Cursor<'encoded> {
    encoded: &'encoded [u8],
    offset: usize,
}

impl<'encoded> Cursor<'encoded> {
    const fn new(encoded: &'encoded [u8]) -> Self {
        Self { encoded, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'encoded [u8], ExternalDecisionSchemaError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(ExternalDecisionSchemaError::Truncated)?;
        let bytes = self
            .encoded
            .get(self.offset..end)
            .ok_or(ExternalDecisionSchemaError::Truncated)?;
        self.offset = end;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], ExternalDecisionSchemaError> {
        self.take(N)?
            .try_into()
            .map_err(|_| ExternalDecisionSchemaError::Truncated)
    }

    fn byte(&mut self) -> Result<u8, ExternalDecisionSchemaError> {
        Ok(self.array::<1>()?[0])
    }

    fn remaining(&self) -> usize {
        self.encoded.len() - self.offset
    }
}
