//! Optimizer module role: executable entrance. External-policy schema v2 entrance.
//!
//! `model` owns the closed request/response vocabulary, `identity` binds every
//! context and per-candidate feature, and `codec` is the strict canonical wire
//! boundary. This entrance alone canonicalizes finite candidate sets and joins
//! them to their point/log identities; it never validates or applies a rewrite.

mod codec;
mod identity;
mod model;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;

use crate::{OptimizationDecisionIdentity, OptimizationRuleIdentity, OptimizationUnitIdentity};

pub use identity::{
    external_psi_decision_schema_v2_identity, psi_target_neutral_decision_target_v2_identity,
};
pub use model::{
    ExternalCandidateFeatures, ExternalDecisionAction, ExternalDecisionContext,
    ExternalDecisionLog, ExternalDecisionPoint, ExternalDecisionSchemaError,
};

use identity::{log_identity, point_identity};

impl ExternalDecisionPoint {
    pub fn new(
        input: OptimizationUnitIdentity,
        rule: OptimizationRuleIdentity,
        legal_candidates: impl IntoIterator<Item = ExternalCandidateFeatures>,
        action: ExternalDecisionAction,
    ) -> Result<Self, ExternalDecisionSchemaError> {
        let mut legal_candidates = legal_candidates.into_iter().collect::<Vec<_>>();
        if legal_candidates.is_empty() {
            return Err(ExternalDecisionSchemaError::EmptyLegalCandidateSet);
        }
        legal_candidates.sort_by_key(ExternalCandidateFeatures::candidate);
        if legal_candidates
            .windows(2)
            .any(|pair| pair[0].candidate() == pair[1].candidate())
        {
            return Err(ExternalDecisionSchemaError::DuplicateCandidate);
        }
        if let ExternalDecisionAction::Choose(candidate) = action
            && !legal_candidates
                .iter()
                .any(|legal| legal.candidate() == candidate)
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
}

impl ExternalDecisionLog {
    pub fn new(
        context: ExternalDecisionContext,
        points: impl IntoIterator<Item = ExternalDecisionPoint>,
    ) -> Result<Self, ExternalDecisionSchemaError> {
        let points = points.into_iter().collect::<Vec<_>>();
        let mut identities = BTreeSet::<OptimizationDecisionIdentity>::new();
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
}
