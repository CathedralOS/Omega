//! Optimizer module role: executable entrance. External-policy schema v2 entrance.
//!
//! This file owns the closed request/response vocabulary, `identity` binds
//! every context and per-candidate feature, and `codec` is the strict canonical
//! wire boundary. This entrance alone canonicalizes finite candidate sets and joins
//! them to their point/log identities; it never validates or applies a rewrite.

mod codec;
mod identity;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;

use crate::{OptimizationDecisionIdentity, OptimizationRuleIdentity, OptimizationUnitIdentity};

pub use identity::{
    external_psi_decision_schema_v2_identity, psi_target_neutral_decision_target_v2_identity,
};

use crate::{
    AnalysisSet, CoreContractDecodeError, OptimizationCandidateIdentity,
    OptimizationDecisionLogIdentity, OptimizationDecisionSchemaIdentity,
    OptimizationDecisionTargetIdentity, OptimizationFactReference,
    OptimizationFactReferenceDecodeError, OptimizationReasonCode, OptimizationRuleSetIdentity,
    OptimizationSelectionIdentity, TargetCostModelIdentity,
};
use crate::{BaselineDecisionOutcome, ValidatedCandidateSummary};
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

/// Authoritative, identity-bearing features for one already validated candidate.
///
/// Analysis bits come from the rule contract. Fact references come from the
/// immutable candidate and are sorted here so insertion order never reaches
/// the external-policy boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalCandidateFeatures {
    summary: ValidatedCandidateSummary,
    consumed_analyses: AnalysisSet,
    consumed_facts: Vec<OptimizationFactReference>,
}

impl ExternalCandidateFeatures {
    pub fn new(
        summary: ValidatedCandidateSummary,
        consumed_analyses: AnalysisSet,
        consumed_facts: impl IntoIterator<Item = OptimizationFactReference>,
    ) -> Result<Self, ExternalDecisionSchemaError> {
        let mut consumed_facts = consumed_facts.into_iter().collect::<Vec<_>>();
        consumed_facts.sort_unstable();
        if consumed_facts.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ExternalDecisionSchemaError::DuplicateCandidateFact);
        }
        Ok(Self {
            summary,
            consumed_analyses,
            consumed_facts,
        })
    }

    pub const fn summary(&self) -> ValidatedCandidateSummary {
        self.summary
    }

    pub const fn candidate(&self) -> OptimizationCandidateIdentity {
        self.summary.candidate
    }

    pub const fn predicted_cost_delta(&self) -> i64 {
        self.summary.predicted_cost_delta
    }

    pub const fn consumed_analyses(&self) -> AnalysisSet {
        self.consumed_analyses
    }

    pub fn consumed_facts(&self) -> &[OptimizationFactReference] {
        &self.consumed_facts
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalDecisionPoint {
    identity: OptimizationDecisionIdentity,
    input: OptimizationUnitIdentity,
    rule: OptimizationRuleIdentity,
    legal_candidates: Vec<ExternalCandidateFeatures>,
    action: ExternalDecisionAction,
}

impl ExternalDecisionPoint {
    pub const fn identity(&self) -> OptimizationDecisionIdentity {
        self.identity
    }

    pub const fn input(&self) -> OptimizationUnitIdentity {
        self.input
    }

    pub const fn rule(&self) -> OptimizationRuleIdentity {
        self.rule
    }

    pub fn legal_candidates(&self) -> &[ExternalCandidateFeatures] {
        &self.legal_candidates
    }

    pub const fn action(&self) -> ExternalDecisionAction {
        self.action
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalDecisionLog {
    identity: OptimizationDecisionLogIdentity,
    context: ExternalDecisionContext,
    points: Vec<ExternalDecisionPoint>,
}

impl ExternalDecisionLog {
    pub const fn identity(&self) -> OptimizationDecisionLogIdentity {
        self.identity
    }

    pub const fn context(&self) -> ExternalDecisionContext {
        self.context
    }

    pub fn points(&self) -> &[ExternalDecisionPoint] {
        &self.points
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalDecisionSchemaError {
    Truncated,
    WrongLogMagic,
    WrongPointMagic,
    UnsupportedLogVersion(u32),
    UnsupportedPointVersion(u32),
    InvalidAnalysisSet(CoreContractDecodeError),
    InvalidFactReference(OptimizationFactReferenceDecodeError),
    EmptyLegalCandidateSet,
    DuplicateCandidate,
    DuplicateCandidateFact,
    NonCanonicalCandidates,
    NonCanonicalCandidateFacts,
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
