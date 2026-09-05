//! Model-free candidate selection, owned by the executing pass manager.

use optimization_core::{
    BaselineDecisionOutcome, OptimizationReasonCode, ValidatedCandidateSummary,
};

pub(super) fn choose_baseline(candidates: &[ValidatedCandidateSummary]) -> BaselineDecisionOutcome {
    candidates
        .iter()
        .min_by_key(|candidate| (candidate.predicted_cost_delta, candidate.candidate))
        .filter(|candidate| candidate.predicted_cost_delta < 0)
        .map_or(
            BaselineDecisionOutcome::Skip(OptimizationReasonCode::NotProfitable),
            |candidate| BaselineDecisionOutcome::Choose(candidate.candidate),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use optimization_core::{
        BaselineDecisionLog, BaselineDecisionLogBuilder, OptimizationCandidateIdentity,
        OptimizationUnitIdentity,
    };

    fn candidate(name: &[u8], cost: i64) -> ValidatedCandidateSummary {
        ValidatedCandidateSummary {
            candidate: OptimizationCandidateIdentity::from_canonical_bytes(name),
            predicted_cost_delta: cost,
        }
    }

    #[test]
    fn order_independent_choice_uses_cost_then_stable_identity() {
        let rows = [
            candidate(b"b", -1),
            candidate(b"a", -2),
            candidate(b"c", -2),
        ];
        let expected = BaselineDecisionOutcome::Choose(rows[1].candidate.min(rows[2].candidate));
        assert_eq!(choose_baseline(&rows), expected);
        assert_eq!(choose_baseline(&[rows[2], rows[1], rows[0]]), expected);
    }

    #[test]
    fn non_improving_candidates_are_replayably_skipped() {
        let input = OptimizationUnitIdentity::from_canonical_bytes(b"input");
        let rows = [candidate(b"same", 0), candidate(b"worse", 1)];
        let outcome = choose_baseline(&rows);
        assert_eq!(
            outcome,
            BaselineDecisionOutcome::Skip(OptimizationReasonCode::NotProfitable)
        );
        let mut records = BaselineDecisionLogBuilder::default();
        records
            .record_validated_outcome(input, rows, outcome)
            .unwrap();
        let log = records.finish();
        assert_eq!(log.records.len(), 1);
        assert_ne!(log.identity.bytes(), [0; 32]);
        assert_eq!(BaselineDecisionLog::decode(&log.encode()), Ok(log));
    }
}
