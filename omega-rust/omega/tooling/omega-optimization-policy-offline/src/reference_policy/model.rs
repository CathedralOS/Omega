use omega_optimization_core::ExternalDecisionAction;

use crate::{DecisionSurfaceIdentity, OfflinePolicyCorpusIdentity, OfflinePolicySplit};

use super::identity::{
    OfflinePolicyAlgorithmIdentity, OfflinePolicyModelIdentity, OfflinePolicyReportIdentity,
    OfflinePolicySplitIdentity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfflinePolicyConfusion {
    pub(super) true_choose: u32,
    pub(super) false_choose: u32,
    pub(super) true_skip: u32,
    pub(super) false_skip: u32,
}

impl OfflinePolicyConfusion {
    pub const fn true_choose(self) -> u32 {
        self.true_choose
    }

    pub const fn false_choose(self) -> u32 {
        self.false_choose
    }

    pub const fn true_skip(self) -> u32 {
        self.true_skip
    }

    pub const fn false_skip(self) -> u32 {
        self.false_skip
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfflinePolicyEvaluationSummary {
    pub(super) decision_count: u32,
    pub(super) recorded_choose_count: u32,
    pub(super) recorded_skip_count: u32,
    pub(super) predicted_choose_count: u32,
    pub(super) predicted_skip_count: u32,
    pub(super) exact_action_match_count: u32,
    pub(super) chosen_candidate_mismatch_count: u32,
    pub(super) confusion: OfflinePolicyConfusion,
    pub(super) selected_predicted_cost_delta: i128,
}

impl OfflinePolicyEvaluationSummary {
    pub const fn decision_count(self) -> u32 {
        self.decision_count
    }

    pub const fn recorded_choose_count(self) -> u32 {
        self.recorded_choose_count
    }

    pub const fn recorded_skip_count(self) -> u32 {
        self.recorded_skip_count
    }

    pub const fn predicted_choose_count(self) -> u32 {
        self.predicted_choose_count
    }

    pub const fn predicted_skip_count(self) -> u32 {
        self.predicted_skip_count
    }

    pub const fn exact_action_match_count(self) -> u32 {
        self.exact_action_match_count
    }

    pub const fn chosen_candidate_mismatch_count(self) -> u32 {
        self.chosen_candidate_mismatch_count
    }

    pub const fn confusion(self) -> OfflinePolicyConfusion {
        self.confusion
    }

    pub const fn selected_predicted_cost_delta(self) -> i128 {
        self.selected_predicted_cost_delta
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostThresholdV1Model {
    pub(super) identity: OfflinePolicyModelIdentity,
    pub(super) corpus: OfflinePolicyCorpusIdentity,
    pub(super) algorithm: OfflinePolicyAlgorithmIdentity,
    pub(super) training_split: OfflinePolicySplitIdentity,
    pub(super) threshold: i128,
    pub(super) training: OfflinePolicyEvaluationSummary,
}

impl CostThresholdV1Model {
    pub const fn identity(&self) -> OfflinePolicyModelIdentity {
        self.identity
    }

    pub const fn corpus(&self) -> OfflinePolicyCorpusIdentity {
        self.corpus
    }

    pub const fn algorithm(&self) -> OfflinePolicyAlgorithmIdentity {
        self.algorithm
    }

    pub const fn training_split(&self) -> OfflinePolicySplitIdentity {
        self.training_split
    }

    pub const fn threshold(&self) -> i128 {
        self.threshold
    }

    pub const fn training_summary(&self) -> OfflinePolicyEvaluationSummary {
        self.training
    }

    pub fn encode(&self) -> Vec<u8> {
        super::codec::encode_model(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfflinePolicyPrediction {
    pub(super) surface: DecisionSurfaceIdentity,
    pub(super) action: ExternalDecisionAction,
    pub(super) selected_predicted_cost_delta: Option<i64>,
}

impl OfflinePolicyPrediction {
    pub const fn surface(self) -> DecisionSurfaceIdentity {
        self.surface
    }

    pub const fn action(self) -> ExternalDecisionAction {
        self.action
    }

    pub const fn selected_predicted_cost_delta(self) -> Option<i64> {
        self.selected_predicted_cost_delta
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflinePolicyEvaluationReport {
    pub(super) identity: OfflinePolicyReportIdentity,
    pub(super) corpus: OfflinePolicyCorpusIdentity,
    pub(super) model: OfflinePolicyModelIdentity,
    pub(super) algorithm: OfflinePolicyAlgorithmIdentity,
    pub(super) split: OfflinePolicySplit,
    pub(super) split_identity: OfflinePolicySplitIdentity,
    pub(super) predictions: Vec<OfflinePolicyPrediction>,
    pub(super) summary: OfflinePolicyEvaluationSummary,
}

impl OfflinePolicyEvaluationReport {
    pub const fn identity(&self) -> OfflinePolicyReportIdentity {
        self.identity
    }

    pub const fn corpus(&self) -> OfflinePolicyCorpusIdentity {
        self.corpus
    }

    pub const fn model(&self) -> OfflinePolicyModelIdentity {
        self.model
    }

    pub const fn algorithm(&self) -> OfflinePolicyAlgorithmIdentity {
        self.algorithm
    }

    pub const fn split(&self) -> OfflinePolicySplit {
        self.split
    }

    pub const fn split_identity(&self) -> OfflinePolicySplitIdentity {
        self.split_identity
    }

    pub fn predictions(&self) -> &[OfflinePolicyPrediction] {
        &self.predictions
    }

    pub const fn summary(&self) -> OfflinePolicyEvaluationSummary {
        self.summary
    }

    pub fn encode(&self) -> Vec<u8> {
        super::codec::encode_report(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfflinePolicyReferenceError {
    EmptySplit(OfflinePolicySplit),
    UnsupportedReportSplit(OfflinePolicySplit),
    CountOverflow,
    AggregateCostOverflow,
    WrongCorpus,
    WrongModel,
    WrongAlgorithm,
    WrongTrainingSplit,
    ModelMismatch,
    ReportMismatch,
    NonCanonicalPredictions,
    IllegalAction,
    Truncated,
    WrongModelMagic,
    WrongReportMagic,
    UnsupportedModelVersion(u32),
    UnsupportedReportVersion(u32),
    UnknownAction(u8),
    UnknownReason(u8),
    ModelIdentityMismatch,
    ReportIdentityMismatch,
    WrongRegressionSplit,
    RegressionReportMismatch,
    RegressionSummaryMismatch,
    WrongRegressionManifestMagic,
    UnsupportedRegressionManifestVersion(u32),
    RegressionManifestIdentityMismatch,
    TrailingBytes,
}

impl std::fmt::Display for OfflinePolicyReferenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid offline reference policy artifact: {self:?}"
        )
    }
}

impl std::error::Error for OfflinePolicyReferenceError {}
