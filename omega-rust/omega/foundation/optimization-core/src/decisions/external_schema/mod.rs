// External optimization decision wire schema v2 (`ExternalDecisionLog`,
// `ExternalDecisionPoint`, `ExternalCandidateFeatures`, `ExternalDecisionContext`).
//
// `ExternalCandidateFeatures::new` sorts and deduplicates consumed fact
// references, and `ExternalDecisionPoint::new` sorts and deduplicates legal
// candidates by `OptimizationCandidateIdentity` while verifying that any
// `Choose(candidate)` action is in the legal candidate set. `write_point_row`
// and `ExternalCandidateFeatures::write_row` emit the shared row bytes for both
// wire (`CountFrame::Wire`) and canonical identity (`CountFrame::Canonical`)
// encodings.

mod codec;
mod identity;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;

use crate::{OptimizationDecisionIdentity, OptimizationRuleIdentity, OptimizationUnitIdentity};

pub use identity::{
    external_psi_decision_schema_identity, psi_target_neutral_decision_target_identity,
};

use crate::optimization_core::CountFrame;
use crate::{
    AnalysisSet, CoreContractDecodeError, OptimizationCandidateIdentity,
    OptimizationDecisionLogIdentity, OptimizationDecisionSchemaIdentity,
    OptimizationDecisionTargetIdentity, OptimizationFactReference,
    OptimizationFactReferenceDecodeError, OptimizationReasonCode, OptimizationRuleSetIdentity,
    OptimizationSelectionIdentity, TargetCostModelIdentity,
};
use crate::{BaselineDecisionOutcome, ValidatedCandidateSummary};
use identity::{encode_action, log_identity, versioned_domain};

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

    pub(super) fn write_row(&self, encoded: &mut Vec<u8>, frame: CountFrame) {
        write_point_row(
            self.input,
            self.rule,
            &self.legal_candidates,
            self.action,
            encoded,
            frame,
        );
    }
}

fn write_point_row(
    input: OptimizationUnitIdentity,
    rule: OptimizationRuleIdentity,
    legal_candidates: &[ExternalCandidateFeatures],
    action: ExternalDecisionAction,
    encoded: &mut Vec<u8>,
    frame: CountFrame,
) {
    encoded.extend_from_slice(&input.bytes());
    encoded.extend_from_slice(&rule.bytes());
    frame.push(encoded, legal_candidates.len());
    for candidate in legal_candidates {
        candidate.write_row(encoded, frame);
    }
    encode_action(encoded, action);
}

fn point_identity(
    input: OptimizationUnitIdentity,
    rule: OptimizationRuleIdentity,
    legal_candidates: &[ExternalCandidateFeatures],
    action: ExternalDecisionAction,
) -> OptimizationDecisionIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(&versioned_domain("omega.external-decision-point", true));
    write_point_row(
        input,
        rule,
        legal_candidates,
        action,
        &mut canonical,
        CountFrame::Canonical,
    );
    OptimizationDecisionIdentity::from_canonical_bytes(&canonical)
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
    schema: OptimizationDecisionSchemaIdentity, // Feature schema digest (`external_psi_decision_schema_identity`).
    source: OptimizationUnitIdentity,           // Source optimization unit digest.
    selections: OptimizationSelectionIdentity,  // Complete build optimization selection digest.
    phase_selections: OptimizationSelectionIdentity, // Phase-local optimization selection digest.
    target: OptimizationDecisionTargetIdentity, // Target context digest (`psi_target_neutral_decision_target_identity` for Psi).
    rule_set: OptimizationRuleSetIdentity,      // Ordered rule-set digest.
    cost_model: TargetCostModelIdentity,        // Cost model digest.
}

crate::record_views! {
    record ExternalDecisionContext; new;
    copied {
        schema: OptimizationDecisionSchemaIdentity,
        source: OptimizationUnitIdentity,
        selections: OptimizationSelectionIdentity,
        phase_selections: OptimizationSelectionIdentity,
        target: OptimizationDecisionTargetIdentity,
        rule_set: OptimizationRuleSetIdentity,
        cost_model: TargetCostModelIdentity,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalCandidateFeatures {
    summary: ValidatedCandidateSummary, // Candidate identity and signed structural cost delta.
    consumed_analyses: AnalysisSet,     // Rule-contract analysis bitset.
    consumed_facts: Vec<OptimizationFactReference>, // Strictly ascending consumed fact references.
}

crate::record_views! {
    record ExternalCandidateFeatures;
    shared {
        summary: ValidatedCandidateSummary,
        consumed_analyses: AnalysisSet,
    }
    slice {
        consumed_facts: OptimizationFactReference,
    }
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

    pub const fn candidate(&self) -> OptimizationCandidateIdentity {
        self.summary.candidate
    }

    pub const fn predicted_cost_delta(&self) -> i64 {
        self.summary.predicted_cost_delta
    }

    pub(super) fn write_row(&self, encoded: &mut Vec<u8>, frame: CountFrame) {
        encoded.extend_from_slice(&self.summary.candidate.bytes());
        encoded.extend_from_slice(&self.summary.predicted_cost_delta.to_le_bytes());
        encoded.extend_from_slice(&self.consumed_analyses.encode());
        frame.push(encoded, self.consumed_facts.len());
        for fact in &self.consumed_facts {
            encoded.extend_from_slice(&fact.encode());
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalDecisionPoint {
    identity: OptimizationDecisionIdentity, // Canonical digest of the decision point row.
    input: OptimizationUnitIdentity,        // Optimization unit identity at this decision point.
    rule: OptimizationRuleIdentity,         // Rule identity evaluated at this point.
    legal_candidates: Vec<ExternalCandidateFeatures>, // Strictly ascending legal candidates.
    action: ExternalDecisionAction,         // Selected candidate or `NotProfitable` skip.
}

crate::record_views! {
    record ExternalDecisionPoint;
    shared {
        identity: OptimizationDecisionIdentity,
        input: OptimizationUnitIdentity,
        rule: OptimizationRuleIdentity,
        action: ExternalDecisionAction,
    }
    slice {
        legal_candidates: ExternalCandidateFeatures,
    }
}

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
    identity: OptimizationDecisionLogIdentity, // Canonical digest of context and ordered point identities.
    context: ExternalDecisionContext,          // Shared policy context for all points in the log.
    points: Vec<ExternalDecisionPoint>,        // Distinct decision points in execution order.
}

crate::record_views! {
    record ExternalDecisionLog;
    shared {
        identity: OptimizationDecisionLogIdentity,
        context: ExternalDecisionContext,
    }
    slice {
        points: ExternalDecisionPoint,
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

crate::invalid_error_display!(
    ExternalDecisionSchemaError,
    "external optimization decision schema"
);
