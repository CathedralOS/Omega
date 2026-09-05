use std::collections::BTreeSet;

use omega_optimization_core::{
    BaselineDecisionOutcome, ExternalCandidateFeatures, ExternalDecisionAction,
    ExternalDecisionContext, ExternalDecisionLog, ExternalDecisionPoint,
};
use omega_optimization_core::{OptimizationRuleIdentity, OptimizationUnitIdentity};

use super::super::ExternalDecisionReplayError;
use super::context;

pub(crate) struct ExternalDecisionReplayCursor<'log> {
    points: &'log [ExternalDecisionPoint],
    #[cfg(test)]
    pub(super) next: usize,
    #[cfg(not(test))]
    next: usize,
}

impl<'log> ExternalDecisionReplayCursor<'log> {
    pub(crate) fn new(
        decisions: &'log ExternalDecisionLog,
        expected_context: ExternalDecisionContext,
    ) -> Result<Self, ExternalDecisionReplayError> {
        if let Some(axis) = context::mismatch(expected_context, decisions.context()) {
            return Err(ExternalDecisionReplayError::ContextMismatch(axis));
        }
        let mut loci = BTreeSet::new();
        for point in decisions.points() {
            if !loci.insert((point.input(), point.rule())) {
                return Err(ExternalDecisionReplayError::DuplicateDecision {
                    input: point.input(),
                    rule: point.rule(),
                });
            }
        }
        Ok(Self {
            points: decisions.points(),
            next: 0,
        })
    }

    pub(crate) fn choose(
        &mut self,
        input: OptimizationUnitIdentity,
        rule: OptimizationRuleIdentity,
        candidates: &[ExternalCandidateFeatures],
    ) -> Result<BaselineDecisionOutcome, ExternalDecisionReplayError> {
        let ordinal = self.next;
        let point =
            self.points
                .get(ordinal)
                .ok_or(ExternalDecisionReplayError::MissingDecision {
                    ordinal,
                    input,
                    rule,
                })?;
        let mut legal_candidates = candidates.to_vec();
        legal_candidates.sort_by_key(ExternalCandidateFeatures::candidate);
        if point.input() != input
            || point.rule() != rule
            || point.legal_candidates() != legal_candidates
        {
            return Err(ExternalDecisionReplayError::IllegalDecision {
                ordinal,
                expected_input: input,
                expected_rule: rule,
            });
        }
        self.next += 1;
        Ok(match point.action() {
            ExternalDecisionAction::Choose(candidate) => BaselineDecisionOutcome::Choose(candidate),
            ExternalDecisionAction::Skip(reason) => BaselineDecisionOutcome::Skip(reason),
        })
    }

    pub(crate) fn require_exhausted(&self) -> Result<(), ExternalDecisionReplayError> {
        let remaining = self.points.len() - self.next;
        if remaining == 0 {
            Ok(())
        } else {
            Err(ExternalDecisionReplayError::LeftoverDecisions {
                first_unused: self.next,
                remaining,
            })
        }
    }

    #[cfg(test)]
    pub(crate) const fn consumed_points(&self) -> usize {
        self.next
    }
}
