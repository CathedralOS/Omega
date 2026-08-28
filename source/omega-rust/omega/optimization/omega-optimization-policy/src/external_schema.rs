use std::collections::BTreeSet;

use omega_optimization_core::{
    OptimizationCandidateIdentity, OptimizationDecisionIdentity, OptimizationDecisionLogIdentity,
    OptimizationDecisionSchemaIdentity, OptimizationDecisionTargetIdentity, OptimizationReasonCode,
    OptimizationRuleIdentity, OptimizationRuleSetIdentity, OptimizationSelectionIdentity,
    OptimizationUnitIdentity, TargetCostModelIdentity,
};

use crate::{BaselineDecisionOutcome, ValidatedCandidateSummary};

const LOG_MAGIC: &[u8; 8] = b"OMGEXD\0\0";
const POINT_MAGIC: &[u8; 8] = b"OMGEXP\0\0";
const VERSION: u32 = 1;

/// Closed v1 feature schema for target-neutral Psi policy decisions.
///
/// The schema consists only of content identities and signed structural cost
/// deltas. It has no representation for paths, authored names, pointers,
/// arena order, diagnostics, or debug strings.
pub fn external_psi_decision_schema_v1_identity() -> OptimizationDecisionSchemaIdentity {
    OptimizationDecisionSchemaIdentity::from_canonical_bytes(
        b"omega.external-psi-decision-schema.v1",
    )
}

/// Psi rules run before target selection. That absence is an explicit context
/// identity rather than an omitted or all-zero target field.
pub fn psi_target_neutral_decision_target_v1_identity() -> OptimizationDecisionTargetIdentity {
    OptimizationDecisionTargetIdentity::from_canonical_bytes(
        b"omega.psi-target-neutral-decision-context.v1",
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalDecisionContext {
    schema: OptimizationDecisionSchemaIdentity,
    source: OptimizationUnitIdentity,
    selections: OptimizationSelectionIdentity,
    phase_selections: OptimizationSelectionIdentity,
    target: OptimizationDecisionTargetIdentity,
    rule_set: OptimizationRuleSetIdentity,
    cost_model: TargetCostModelIdentity,
}

impl ExternalDecisionContext {
    pub const fn new(
        schema: OptimizationDecisionSchemaIdentity,
        source: OptimizationUnitIdentity,
        selections: OptimizationSelectionIdentity,
        phase_selections: OptimizationSelectionIdentity,
        target: OptimizationDecisionTargetIdentity,
        rule_set: OptimizationRuleSetIdentity,
        cost_model: TargetCostModelIdentity,
    ) -> Self {
        Self {
            schema,
            source,
            selections,
            phase_selections,
            target,
            rule_set,
            cost_model,
        }
    }

    pub const fn schema(self) -> OptimizationDecisionSchemaIdentity {
        self.schema
    }

    pub const fn source(self) -> OptimizationUnitIdentity {
        self.source
    }

    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }

    pub const fn phase_selections(self) -> OptimizationSelectionIdentity {
        self.phase_selections
    }

    pub const fn target(self) -> OptimizationDecisionTargetIdentity {
        self.target
    }

    pub const fn rule_set(self) -> OptimizationRuleSetIdentity {
        self.rule_set
    }

    pub const fn cost_model(self) -> TargetCostModelIdentity {
        self.cost_model
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalDecisionPoint {
    identity: OptimizationDecisionIdentity,
    input: OptimizationUnitIdentity,
    rule: OptimizationRuleIdentity,
    legal_candidates: Vec<ValidatedCandidateSummary>,
    action: ExternalDecisionAction,
}

/// One member of a decision point's finite action set. Every point admits
/// `Choose` for each listed candidate plus the explicit model-free skip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalDecisionAction {
    Choose(OptimizationCandidateIdentity),
    Skip(OptimizationReasonCode),
}

impl From<BaselineDecisionOutcome> for ExternalDecisionAction {
    fn from(outcome: BaselineDecisionOutcome) -> Self {
        match outcome {
            BaselineDecisionOutcome::Choose(candidate) => Self::Choose(candidate),
            BaselineDecisionOutcome::Skip(reason) => Self::Skip(reason),
        }
    }
}

impl ExternalDecisionPoint {
    pub fn new(
        input: OptimizationUnitIdentity,
        rule: OptimizationRuleIdentity,
        legal_candidates: impl IntoIterator<Item = ValidatedCandidateSummary>,
        action: ExternalDecisionAction,
    ) -> Result<Self, ExternalDecisionSchemaError> {
        let mut legal_candidates = legal_candidates.into_iter().collect::<Vec<_>>();
        if legal_candidates.is_empty() {
            return Err(ExternalDecisionSchemaError::EmptyLegalCandidateSet);
        }
        legal_candidates.sort_by_key(|candidate| candidate.candidate);
        if legal_candidates
            .windows(2)
            .any(|pair| pair[0].candidate == pair[1].candidate)
        {
            return Err(ExternalDecisionSchemaError::DuplicateCandidate);
        }
        if let ExternalDecisionAction::Choose(candidate) = action
            && !legal_candidates
                .iter()
                .any(|legal| legal.candidate == candidate)
        {
            return Err(ExternalDecisionSchemaError::IllegalAction);
        }
        let identity = point_identity(input, rule, &legal_candidates, action);
        Ok(Self {
            identity,
            input,
            rule,
            legal_candidates,
            action,
        })
    }

    pub const fn identity(&self) -> OptimizationDecisionIdentity {
        self.identity
    }

    pub const fn input(&self) -> OptimizationUnitIdentity {
        self.input
    }

    pub const fn rule(&self) -> OptimizationRuleIdentity {
        self.rule
    }

    pub fn legal_candidates(&self) -> &[ValidatedCandidateSummary] {
        &self.legal_candidates
    }

    pub const fn action(&self) -> ExternalDecisionAction {
        self.action
    }

    fn encode(&self) -> Vec<u8> {
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
            encoded.extend_from_slice(&candidate.candidate.bytes());
            encoded.extend_from_slice(&candidate.predicted_cost_delta.to_le_bytes());
        }
        encode_action(&mut encoded, self.action);
        encoded
    }

    fn decode(encoded: &[u8]) -> Result<Self, ExternalDecisionSchemaError> {
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
        if count > cursor.remaining().saturating_sub(2) / 40 {
            return Err(ExternalDecisionSchemaError::Truncated);
        }
        let mut candidates = Vec::with_capacity(count);
        for _ in 0..count {
            candidates.push(ValidatedCandidateSummary {
                candidate: OptimizationCandidateIdentity::from_bytes(cursor.array()?),
                predicted_cost_delta: i64::from_le_bytes(cursor.array()?),
            });
        }
        if candidates
            .windows(2)
            .any(|pair| pair[0].candidate >= pair[1].candidate)
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalDecisionLog {
    identity: OptimizationDecisionLogIdentity,
    context: ExternalDecisionContext,
    points: Vec<ExternalDecisionPoint>,
}

impl ExternalDecisionLog {
    pub fn new(
        context: ExternalDecisionContext,
        points: impl IntoIterator<Item = ExternalDecisionPoint>,
    ) -> Result<Self, ExternalDecisionSchemaError> {
        let points = points.into_iter().collect::<Vec<_>>();
        let mut identities = BTreeSet::new();
        if points
            .iter()
            .any(|point| !identities.insert(point.identity))
        {
            return Err(ExternalDecisionSchemaError::DuplicateDecisionPoint);
        }
        let identity = log_identity(context, &points);
        Ok(Self {
            identity,
            context,
            points,
        })
    }

    pub const fn identity(&self) -> OptimizationDecisionLogIdentity {
        self.identity
    }

    pub const fn context(&self) -> ExternalDecisionContext {
        self.context
    }

    pub fn points(&self) -> &[ExternalDecisionPoint] {
        &self.points
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalDecisionSchemaError {
    Truncated,
    WrongLogMagic,
    WrongPointMagic,
    UnsupportedLogVersion(u32),
    UnsupportedPointVersion(u32),
    EmptyLegalCandidateSet,
    DuplicateCandidate,
    NonCanonicalCandidates,
    IllegalAction,
    UnknownAction(u8),
    UnknownReason(u8),
    DuplicateDecisionPoint,
    PointIdentityMismatch,
    LogIdentityMismatch,
    TrailingBytes,
}

impl std::fmt::Display for ExternalDecisionSchemaError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid external optimization decision schema: {self:?}"
        )
    }
}

impl std::error::Error for ExternalDecisionSchemaError {}

fn point_identity(
    input: OptimizationUnitIdentity,
    rule: OptimizationRuleIdentity,
    candidates: &[ValidatedCandidateSummary],
    action: ExternalDecisionAction,
) -> OptimizationDecisionIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.external-decision-point.v1\0");
    canonical.extend_from_slice(&input.bytes());
    canonical.extend_from_slice(&rule.bytes());
    canonical.extend_from_slice(
        &u64::try_from(candidates.len())
            .expect("external decision candidate count fits u64")
            .to_le_bytes(),
    );
    for candidate in candidates {
        canonical.extend_from_slice(&candidate.candidate.bytes());
        canonical.extend_from_slice(&candidate.predicted_cost_delta.to_le_bytes());
    }
    encode_action(&mut canonical, action);
    OptimizationDecisionIdentity::from_canonical_bytes(&canonical)
}

fn log_identity(
    context: ExternalDecisionContext,
    points: &[ExternalDecisionPoint],
) -> OptimizationDecisionLogIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.external-decision-log.v1\0");
    encode_context(&mut canonical, context);
    canonical.extend_from_slice(
        &u64::try_from(points.len())
            .expect("external decision point count fits u64")
            .to_le_bytes(),
    );
    for point in points {
        canonical.extend_from_slice(&point.identity.bytes());
    }
    OptimizationDecisionLogIdentity::from_canonical_bytes(&canonical)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(name: &[u8], cost: i64) -> ValidatedCandidateSummary {
        ValidatedCandidateSummary {
            candidate: OptimizationCandidateIdentity::from_canonical_bytes(name),
            predicted_cost_delta: cost,
        }
    }

    fn context() -> ExternalDecisionContext {
        ExternalDecisionContext::new(
            external_psi_decision_schema_v1_identity(),
            OptimizationUnitIdentity::from_canonical_bytes(b"source"),
            OptimizationSelectionIdentity::from_bytes([1; 32]),
            OptimizationSelectionIdentity::from_bytes([2; 32]),
            psi_target_neutral_decision_target_v1_identity(),
            OptimizationRuleSetIdentity::from_canonical_bytes(b"rules"),
            TargetCostModelIdentity::from_canonical_bytes(b"cost"),
        )
    }

    fn point() -> ExternalDecisionPoint {
        let first = candidate(b"first", -1);
        let second = candidate(b"second", -2);
        ExternalDecisionPoint::new(
            OptimizationUnitIdentity::from_canonical_bytes(b"input"),
            OptimizationRuleIdentity::from_canonical_bytes(b"rule"),
            [first, second],
            ExternalDecisionAction::Choose(second.candidate),
        )
        .unwrap()
    }

    #[test]
    fn point_canonicalizes_legal_actions_without_changing_policy_outcome() {
        let rows = [candidate(b"slow", -1), candidate(b"fast", -3)];
        let mut baseline = crate::BaselinePolicy::default();
        let outcome = baseline.choose(
            OptimizationUnitIdentity::from_canonical_bytes(b"input"),
            rows,
        );
        let point = ExternalDecisionPoint::new(
            OptimizationUnitIdentity::from_canonical_bytes(b"input"),
            OptimizationRuleIdentity::from_canonical_bytes(b"rule"),
            rows.into_iter().rev(),
            outcome.into(),
        )
        .unwrap();
        assert_eq!(point.action(), outcome.into());
        assert!(
            point
                .legal_candidates()
                .windows(2)
                .all(|pair| { pair[0].candidate < pair[1].candidate })
        );
    }

    #[test]
    fn strict_log_round_trip_binds_every_context_axis_and_point_order() {
        let first = point();
        let second = ExternalDecisionPoint::new(
            OptimizationUnitIdentity::from_canonical_bytes(b"next"),
            OptimizationRuleIdentity::from_canonical_bytes(b"other-rule"),
            [candidate(b"only", 0)],
            ExternalDecisionAction::Skip(OptimizationReasonCode::NotProfitable),
        )
        .unwrap();
        let log = ExternalDecisionLog::new(context(), [first.clone(), second.clone()]).unwrap();
        assert_eq!(ExternalDecisionLog::decode(&log.encode()), Ok(log.clone()));
        let reversed = ExternalDecisionLog::new(context(), [second, first]).unwrap();
        assert_ne!(log.identity(), reversed.identity());

        for offset in [44, 76, 108, 140, 172, 204, 236] {
            let mut corrupt = log.encode();
            corrupt[offset] ^= 1;
            assert_eq!(
                ExternalDecisionLog::decode(&corrupt),
                Err(ExternalDecisionSchemaError::LogIdentityMismatch)
            );
        }
    }

    #[test]
    fn illegal_duplicate_and_noncanonical_actions_reject() {
        let candidate = candidate(b"candidate", -1);
        assert_eq!(
            ExternalDecisionPoint::new(
                OptimizationUnitIdentity::from_canonical_bytes(b"input"),
                OptimizationRuleIdentity::from_canonical_bytes(b"rule"),
                [candidate, candidate],
                ExternalDecisionAction::Choose(candidate.candidate),
            ),
            Err(ExternalDecisionSchemaError::DuplicateCandidate)
        );
        assert_eq!(
            ExternalDecisionPoint::new(
                OptimizationUnitIdentity::from_canonical_bytes(b"input"),
                OptimizationRuleIdentity::from_canonical_bytes(b"rule"),
                [candidate],
                ExternalDecisionAction::Choose(
                    OptimizationCandidateIdentity::from_canonical_bytes(b"foreign"),
                ),
            ),
            Err(ExternalDecisionSchemaError::IllegalAction)
        );

        let mut encoded = point().encode();
        let candidate_start = 8 + 4 + 32 + 32 + 32 + 4;
        let first = encoded[candidate_start..candidate_start + 40].to_vec();
        let second = encoded[candidate_start + 40..candidate_start + 80].to_vec();
        encoded[candidate_start..candidate_start + 40].copy_from_slice(&second);
        encoded[candidate_start + 40..candidate_start + 80].copy_from_slice(&first);
        assert_eq!(
            ExternalDecisionPoint::decode(&encoded),
            Err(ExternalDecisionSchemaError::NonCanonicalCandidates)
        );
    }

    #[test]
    fn codec_rejects_framing_tamper_and_duplicate_points() {
        let duplicated = point();
        assert_eq!(
            ExternalDecisionLog::new(context(), [duplicated.clone(), duplicated]),
            Err(ExternalDecisionSchemaError::DuplicateDecisionPoint)
        );
        let log = ExternalDecisionLog::new(context(), [point()]).unwrap();
        let encoded = log.encode();
        assert_eq!(
            ExternalDecisionLog::decode(&encoded[..encoded.len() - 1]),
            Err(ExternalDecisionSchemaError::Truncated)
        );
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert_eq!(
            ExternalDecisionLog::decode(&trailing),
            Err(ExternalDecisionSchemaError::TrailingBytes)
        );
        let mut wrong_version = encoded;
        wrong_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(
            ExternalDecisionLog::decode(&wrong_version),
            Err(ExternalDecisionSchemaError::UnsupportedLogVersion(2))
        );
    }
}
