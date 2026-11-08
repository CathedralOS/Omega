use crate::cursor::Cursor;
use crate::optimization_core::CountFrame;
use crate::{
    OptimizationCandidateIdentity, OptimizationDecisionIdentity, OptimizationDecisionLogIdentity,
    OptimizationReasonCode, OptimizationUnitIdentity,
};

/// Domain prefix of one baseline decision's canonical identity encoding.
const BASELINE_DECISION_DOMAIN: &[u8] = b"omega.baseline-decision.v1\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValidatedCandidateSummary {
    pub candidate: OptimizationCandidateIdentity,
    pub predicted_cost_delta: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaselineDecisionOutcome {
    Choose(OptimizationCandidateIdentity),
    Skip(OptimizationReasonCode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaselineDecisionRecordError {
    EmptyCandidateSet,
    UnknownCandidate(OptimizationCandidateIdentity),
    UnsupportedSkipReason(OptimizationReasonCode),
}

crate::invalid_error_display!(
    BaselineDecisionRecordError,
    "recorded optimization decision"
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineDecisionRecord {
    pub identity: OptimizationDecisionIdentity,
    pub input: OptimizationUnitIdentity,
    pub considered: Vec<ValidatedCandidateSummary>,
    pub outcome: BaselineDecisionOutcome,
}

impl BaselineDecisionRecord {
    /// The authoritative row, written once for both encodings. The wire log
    /// frames the considered-candidate count as u32 and the canonical identity
    /// row as u64; every other byte and the field order are stated here alone.
    fn write_row(&self, encoded: &mut Vec<u8>, frame: CountFrame) {
        encoded.extend_from_slice(&self.input.bytes());
        frame.push(encoded, self.considered.len());
        for candidate in &self.considered {
            encoded.extend_from_slice(&candidate.candidate.bytes());
            encoded.extend_from_slice(&candidate.predicted_cost_delta.to_le_bytes());
        }
        match self.outcome {
            BaselineDecisionOutcome::Choose(candidate) => {
                encoded.push(1);
                encoded.extend_from_slice(&candidate.bytes());
            }
            BaselineDecisionOutcome::Skip(reason) => {
                encoded.push(2);
                encoded.push(reason as u8);
            }
        }
    }

    fn canonical_identity(&self) -> OptimizationDecisionIdentity {
        let mut canonical = Vec::new();
        canonical.extend_from_slice(BASELINE_DECISION_DOMAIN);
        self.write_row(&mut canonical, CountFrame::Canonical);
        OptimizationDecisionIdentity::from_canonical_bytes(&canonical)
    }

    /// Row builder for the one place a decision is recorded before its identity
    /// is known: the identity is a function of the row, never an input.
    fn record(
        input: OptimizationUnitIdentity,
        considered: Vec<ValidatedCandidateSummary>,
        outcome: BaselineDecisionOutcome,
    ) -> Self {
        let mut record = Self {
            identity: OptimizationDecisionIdentity::from_bytes([0; 32]),
            input,
            considered,
            outcome,
        };
        record.identity = record.canonical_identity();
        record
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineDecisionLog {
    pub identity: OptimizationDecisionLogIdentity,
    pub records: Vec<BaselineDecisionRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaselineDecisionLogDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownOutcome(u8),
    UnknownReason(u8),
    DecisionIdentityMismatch,
    TrailingBytes,
}

crate::invalid_error_display!(BaselineDecisionLogDecodeError, "baseline decision log");

impl BaselineDecisionLog {
    /// Concatenate independently replayable pass-local logs in exact execution
    /// order and derive one pipeline-level identity over their decision rows.
    pub fn concatenate<'log>(
        logs: impl IntoIterator<Item = &'log Self>,
    ) -> Result<Self, BaselineDecisionLogDecodeError> {
        let mut records = Vec::new();
        for log in logs {
            if Self::decode(&log.encode())? != *log {
                return Err(BaselineDecisionLogDecodeError::DecisionIdentityMismatch);
            }
            records.extend(log.records.iter().cloned());
        }
        Ok(finish_log(records))
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut encoded = Vec::new();
        encoded.extend_from_slice(b"OMGBDL\0\0");
        crate::codec_bytes::push_encoded(&mut encoded, 1_u32.to_le_bytes());
        crate::codec_bytes::push_count_u32(&mut encoded, self.records.len());
        for record in &self.records {
            encoded.extend_from_slice(&record.identity.bytes());
            record.write_row(&mut encoded, CountFrame::Wire);
        }
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, BaselineDecisionLogDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != b"OMGBDL\0\0" {
            return Err(BaselineDecisionLogDecodeError::WrongMagic);
        }
        let version = cursor.u32()?;
        if version != 1 {
            return Err(BaselineDecisionLogDecodeError::UnsupportedVersion(version));
        }
        let count = cursor.u32()?;
        let mut records = Vec::new();
        for _ in 0..count {
            let encoded_identity = OptimizationDecisionIdentity::from_bytes(cursor.array()?);
            let input = OptimizationUnitIdentity::from_bytes(cursor.array()?);
            let candidate_count = cursor.u32()?;
            let mut considered = Vec::with_capacity(candidate_count as usize);
            for _ in 0..candidate_count {
                considered.push(ValidatedCandidateSummary {
                    candidate: OptimizationCandidateIdentity::from_bytes(cursor.array()?),
                    predicted_cost_delta: cursor.i64()?,
                });
            }
            let outcome = match cursor.byte()? {
                1 => BaselineDecisionOutcome::Choose(OptimizationCandidateIdentity::from_bytes(
                    cursor.array()?,
                )),
                2 => BaselineDecisionOutcome::Skip(match cursor.byte()? {
                    reason if reason == OptimizationReasonCode::NotProfitable as u8 => {
                        OptimizationReasonCode::NotProfitable
                    }
                    reason => return Err(BaselineDecisionLogDecodeError::UnknownReason(reason)),
                }),
                outcome => return Err(BaselineDecisionLogDecodeError::UnknownOutcome(outcome)),
            };
            let canonical = BaselineDecisionRecord::record(input, considered, outcome);
            if canonical.identity != encoded_identity {
                return Err(BaselineDecisionLogDecodeError::DecisionIdentityMismatch);
            }
            records.push(canonical);
        }
        if cursor.offset != encoded.len() {
            return Err(BaselineDecisionLogDecodeError::TrailingBytes);
        }
        Ok(finish_log(records))
    }
}

/// Record-only builder. It never chooses a candidate or applies a rewrite.
#[derive(Debug, Default)]
pub struct BaselineDecisionLogBuilder {
    records: Vec<BaselineDecisionRecord>,
}

impl BaselineDecisionLogBuilder {
    /// Retain a supplied outcome after the ordinary candidate validators have
    /// accepted the complete roster. This checks membership, not profitability;
    /// it neither runs a policy nor establishes candidate validity.
    /// This uses the baseline log's existing canonical representation so the
    /// actual selected action remains the identity-bundle authority.
    pub fn record_validated_outcome(
        &mut self,
        input: OptimizationUnitIdentity,
        candidates: impl IntoIterator<Item = ValidatedCandidateSummary>,
        outcome: BaselineDecisionOutcome,
    ) -> Result<BaselineDecisionOutcome, BaselineDecisionRecordError> {
        let considered = canonicalize_candidates(candidates);
        if considered.is_empty() {
            return Err(BaselineDecisionRecordError::EmptyCandidateSet);
        }
        match outcome {
            BaselineDecisionOutcome::Choose(candidate)
                if !considered
                    .iter()
                    .any(|considered| considered.candidate == candidate) =>
            {
                return Err(BaselineDecisionRecordError::UnknownCandidate(candidate));
            }
            BaselineDecisionOutcome::Skip(reason)
                if reason != OptimizationReasonCode::NotProfitable =>
            {
                return Err(BaselineDecisionRecordError::UnsupportedSkipReason(reason));
            }
            BaselineDecisionOutcome::Choose(_) | BaselineDecisionOutcome::Skip(_) => {}
        }
        self.record_canonical(input, considered, outcome);
        Ok(outcome)
    }

    fn record_canonical(
        &mut self,
        input: OptimizationUnitIdentity,
        considered: Vec<ValidatedCandidateSummary>,
        outcome: BaselineDecisionOutcome,
    ) {
        self.records
            .push(BaselineDecisionRecord::record(input, considered, outcome));
    }

    pub fn finish(self) -> BaselineDecisionLog {
        finish_log(self.records)
    }
}

fn canonicalize_candidates(
    candidates: impl IntoIterator<Item = ValidatedCandidateSummary>,
) -> Vec<ValidatedCandidateSummary> {
    let mut considered = candidates.into_iter().collect::<Vec<_>>();
    considered.sort_by_key(|candidate| (candidate.predicted_cost_delta, candidate.candidate));
    considered.dedup_by_key(|candidate| candidate.candidate);
    considered
}

fn finish_log(records: Vec<BaselineDecisionRecord>) -> BaselineDecisionLog {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.baseline-decision-log.v1\0");
    crate::codec_bytes::push_count_u64(&mut canonical, records.len());
    for record in &records {
        canonical.extend_from_slice(&record.identity.bytes());
    }
    BaselineDecisionLog {
        identity: OptimizationDecisionLogIdentity::from_canonical_bytes(&canonical),
        records,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BaselineDecisionLog, BaselineDecisionLogBuilder, BaselineDecisionLogDecodeError,
        BaselineDecisionOutcome, BaselineDecisionRecordError, OptimizationCandidateIdentity,
        OptimizationUnitIdentity, ValidatedCandidateSummary,
    };

    fn candidate(name: &[u8], cost: i64) -> ValidatedCandidateSummary {
        ValidatedCandidateSummary {
            candidate: OptimizationCandidateIdentity::from_canonical_bytes(name),
            predicted_cost_delta: cost,
        }
    }

    #[test]
    fn recorded_validated_outcome_can_choose_a_non_baseline_member() {
        let input = OptimizationUnitIdentity::from_canonical_bytes(b"input");
        let baseline_choice = candidate(b"baseline", -2);
        let replay_choice = candidate(b"replay", -1);
        let mut policy = BaselineDecisionLogBuilder::default();
        assert_eq!(
            policy.record_validated_outcome(
                input,
                [baseline_choice, replay_choice],
                BaselineDecisionOutcome::Choose(replay_choice.candidate),
            ),
            Ok(BaselineDecisionOutcome::Choose(replay_choice.candidate))
        );
        let log = policy.finish();
        assert_eq!(
            log.records[0].considered,
            vec![baseline_choice, replay_choice]
        );
        assert_eq!(BaselineDecisionLog::decode(&log.encode()), Ok(log));
    }

    #[test]
    fn recorded_validated_outcome_rejects_foreign_choice() {
        let mut policy = BaselineDecisionLogBuilder::default();
        let foreign = OptimizationCandidateIdentity::from_canonical_bytes(b"foreign");
        assert_eq!(
            policy.record_validated_outcome(
                OptimizationUnitIdentity::from_canonical_bytes(b"input"),
                [candidate(b"legal", -1)],
                BaselineDecisionOutcome::Choose(foreign),
            ),
            Err(BaselineDecisionRecordError::UnknownCandidate(foreign))
        );
    }

    #[test]
    fn codec_rejects_tamper_and_trailing_bytes() {
        let input = OptimizationUnitIdentity::from_canonical_bytes(b"input");
        let mut policy = BaselineDecisionLogBuilder::default();
        policy
            .record_validated_outcome(
                input,
                [candidate(b"better", -1)],
                BaselineDecisionOutcome::Choose(candidate(b"better", -1).candidate),
            )
            .unwrap();
        let log = policy.finish();
        let mut tampered = log.encode();
        tampered[20] ^= 1;
        assert_eq!(
            BaselineDecisionLog::decode(&tampered),
            Err(BaselineDecisionLogDecodeError::DecisionIdentityMismatch)
        );
        let mut trailing = log.encode();
        trailing.push(0);
        assert_eq!(
            BaselineDecisionLog::decode(&trailing),
            Err(BaselineDecisionLogDecodeError::TrailingBytes)
        );
    }

    #[test]
    fn concatenation_replays_pass_order_across_empty_logs() {
        let input = OptimizationUnitIdentity::from_canonical_bytes(b"input");
        let mut first = BaselineDecisionLogBuilder::default();
        first
            .record_validated_outcome(
                input,
                [candidate(b"first", -2)],
                BaselineDecisionOutcome::Choose(candidate(b"first", -2).candidate),
            )
            .unwrap();
        let first = first.finish();
        let empty = BaselineDecisionLogBuilder::default().finish();
        let mut second = BaselineDecisionLogBuilder::default();
        second
            .record_validated_outcome(
                input,
                [candidate(b"second", -1)],
                BaselineDecisionOutcome::Choose(candidate(b"second", -1).candidate),
            )
            .unwrap();
        let second = second.finish();

        let combined = BaselineDecisionLog::concatenate([&first, &empty, &second]).unwrap();
        assert_eq!(combined.records.len(), 2);
        assert_eq!(combined.records[0], first.records[0]);
        assert_eq!(combined.records[1], second.records[0]);
        assert_eq!(
            BaselineDecisionLog::decode(&combined.encode()),
            Ok(combined.clone())
        );

        let reversed = BaselineDecisionLog::concatenate([&second, &empty, &first]).unwrap();
        assert_ne!(reversed.identity, combined.identity);
    }
}

crate::truncated_from_adapter!(BaselineDecisionLogDecodeError);
