use optimization_core::ExternalDecisionAction;
use sha2::{Digest, Sha256};

use crate::{OfflinePolicySplit, ValidatedOfflinePolicyCorpus};

use super::model::{
    CostThresholdV1Model, OfflinePolicyEvaluationReport, OfflinePolicyEvaluationSummary,
    OfflinePolicyPrediction,
};

macro_rules! identity_type {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            pub const fn bytes(self) -> [u8; 32] {
                self.0
            }
        }
    };
}

identity_type!(OfflinePolicyAlgorithmIdentity);
identity_type!(OfflinePolicySplitIdentity);
identity_type!(OfflinePolicyModelIdentity);
identity_type!(OfflinePolicyReportIdentity);

pub fn cost_threshold_v1_algorithm_identity() -> OfflinePolicyAlgorithmIdentity {
    let mut digest = Sha256::new();
    digest.update(b"omega.offline-policy.algorithm.cost-threshold.v1\0");
    OfflinePolicyAlgorithmIdentity(digest.finalize().into())
}

pub fn offline_policy_split_identity(
    corpus: &ValidatedOfflinePolicyCorpus,
    split: OfflinePolicySplit,
) -> OfflinePolicySplitIdentity {
    let mut digest = Sha256::new();
    digest.update(b"omega.offline-policy.split.sha256.v1\0");
    digest.update(corpus.identity().bytes());
    digest.update([split.tag()]);
    let count = corpus
        .examples()
        .iter()
        .filter(|example| example.split() == split)
        .count();
    digest.update((count as u64).to_le_bytes());
    for example in corpus
        .examples()
        .iter()
        .filter(|example| example.split() == split)
    {
        digest.update(example.surface().bytes());
        digest.update(example.log().bytes());
        digest.update(example.point_ordinal().to_le_bytes());
        digest.update(example.source().bytes());
        digest.update(example.point().identity().bytes());
    }
    OfflinePolicySplitIdentity(digest.finalize().into())
}

pub(super) fn model_identity(model: &CostThresholdV1Model) -> OfflinePolicyModelIdentity {
    let mut digest = Sha256::new();
    digest.update(b"omega.offline-policy.model.cost-threshold.sha256.v1\0");
    digest.update(model.corpus.bytes());
    digest.update(model.algorithm.bytes());
    digest.update(model.training_split.bytes());
    digest.update(model.threshold.to_le_bytes());
    encode_summary(&mut digest, model.training);
    OfflinePolicyModelIdentity(digest.finalize().into())
}

pub(super) fn report_identity(
    report: &OfflinePolicyEvaluationReport,
) -> OfflinePolicyReportIdentity {
    let mut digest = Sha256::new();
    digest.update(b"omega.offline-policy.report.cost-threshold.sha256.v1\0");
    digest.update(report.corpus.bytes());
    digest.update(report.model.bytes());
    digest.update(report.algorithm.bytes());
    digest.update([report.split.tag()]);
    digest.update(report.split_identity.bytes());
    digest.update((report.predictions.len() as u64).to_le_bytes());
    for prediction in &report.predictions {
        encode_prediction(&mut digest, *prediction);
    }
    encode_summary(&mut digest, report.summary);
    OfflinePolicyReportIdentity(digest.finalize().into())
}

pub(super) fn encode_summary(digest: &mut Sha256, summary: OfflinePolicyEvaluationSummary) {
    digest.update(summary.decision_count.to_le_bytes());
    digest.update(summary.recorded_choose_count.to_le_bytes());
    digest.update(summary.recorded_skip_count.to_le_bytes());
    digest.update(summary.predicted_choose_count.to_le_bytes());
    digest.update(summary.predicted_skip_count.to_le_bytes());
    digest.update(summary.exact_action_match_count.to_le_bytes());
    digest.update(summary.chosen_candidate_mismatch_count.to_le_bytes());
    digest.update(summary.confusion.true_choose.to_le_bytes());
    digest.update(summary.confusion.false_choose.to_le_bytes());
    digest.update(summary.confusion.true_skip.to_le_bytes());
    digest.update(summary.confusion.false_skip.to_le_bytes());
    digest.update(summary.selected_predicted_cost_delta.to_le_bytes());
}

fn encode_prediction(digest: &mut Sha256, prediction: OfflinePolicyPrediction) {
    digest.update(prediction.surface.bytes());
    match prediction.action {
        ExternalDecisionAction::Choose(candidate) => {
            digest.update([1]);
            digest.update(candidate.bytes());
        }
        ExternalDecisionAction::Skip(reason) => {
            digest.update([2]);
            digest.update([reason as u8]);
        }
    }
    match prediction.selected_predicted_cost_delta {
        Some(cost) => {
            digest.update([1]);
            digest.update(cost.to_le_bytes());
        }
        None => digest.update([0]),
    }
}
