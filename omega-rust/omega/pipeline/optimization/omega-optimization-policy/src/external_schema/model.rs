use omega_optimization_core::{
    AnalysisSet, CoreContractDecodeError, OptimizationCandidateIdentity,
    OptimizationDecisionIdentity, OptimizationDecisionLogIdentity,
    OptimizationDecisionSchemaIdentity, OptimizationDecisionTargetIdentity,
    OptimizationFactReference, OptimizationFactReferenceDecodeError, OptimizationReasonCode,
    OptimizationRuleIdentity, OptimizationRuleSetIdentity, OptimizationSelectionIdentity,
    OptimizationUnitIdentity, TargetCostModelIdentity,
};

use crate::{BaselineDecisionOutcome, ValidatedCandidateSummary};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalDecisionContext {
    pub(super) schema: OptimizationDecisionSchemaIdentity,
    pub(super) source: OptimizationUnitIdentity,
    pub(super) selections: OptimizationSelectionIdentity,
    pub(super) phase_selections: OptimizationSelectionIdentity,
    pub(super) target: OptimizationDecisionTargetIdentity,
    pub(super) rule_set: OptimizationRuleSetIdentity,
    pub(super) cost_model: TargetCostModelIdentity,
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
    pub(super) summary: ValidatedCandidateSummary,
    pub(super) consumed_analyses: AnalysisSet,
    pub(super) consumed_facts: Vec<OptimizationFactReference>,
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
    pub(super) identity: OptimizationDecisionIdentity,
    pub(super) input: OptimizationUnitIdentity,
    pub(super) rule: OptimizationRuleIdentity,
    pub(super) legal_candidates: Vec<ExternalCandidateFeatures>,
    pub(super) action: ExternalDecisionAction,
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
    pub(super) identity: OptimizationDecisionLogIdentity,
    pub(super) context: ExternalDecisionContext,
    pub(super) points: Vec<ExternalDecisionPoint>,
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
