use crate::{OfflinePolicyCorpusIdentity, OfflinePolicySplit, ValidatedOfflinePolicyCorpus};

use super::{codec, identity};
use crate::reference_policy::{
    CostThresholdV1Model, OfflinePolicyAlgorithmIdentity, OfflinePolicyEvaluationSummary,
    OfflinePolicyModelIdentity, OfflinePolicyReferenceError, OfflinePolicyReportIdentity,
    OfflinePolicySplitIdentity, evaluation,
};

use super::identity::OfflinePolicyRegressionManifestIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflinePolicyRegressionManifest {
    pub(super) identity: OfflinePolicyRegressionManifestIdentity,
    pub(super) corpus: OfflinePolicyCorpusIdentity,
    pub(super) model: OfflinePolicyModelIdentity,
    pub(super) algorithm: OfflinePolicyAlgorithmIdentity,
    pub(super) regression_split: OfflinePolicySplitIdentity,
    pub(super) expected_report: OfflinePolicyReportIdentity,
    pub(super) expected_summary: OfflinePolicyEvaluationSummary,
}

impl OfflinePolicyRegressionManifest {
    pub const fn identity(&self) -> OfflinePolicyRegressionManifestIdentity {
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

    pub const fn regression_split(&self) -> OfflinePolicySplitIdentity {
        self.regression_split
    }

    pub const fn expected_report(&self) -> OfflinePolicyReportIdentity {
        self.expected_report
    }

    pub const fn expected_summary(&self) -> OfflinePolicyEvaluationSummary {
        self.expected_summary
    }

    pub fn encode(&self) -> Vec<u8> {
        codec::encode(self)
    }
}

pub(super) fn create(
    corpus: &ValidatedOfflinePolicyCorpus,
    model: &CostThresholdV1Model,
) -> Result<OfflinePolicyRegressionManifest, OfflinePolicyReferenceError> {
    let report = evaluation::evaluate(corpus, model, OfflinePolicySplit::Regression)?;
    let mut manifest = OfflinePolicyRegressionManifest {
        identity: OfflinePolicyRegressionManifestIdentity::from_bytes([0; 32]),
        corpus: corpus.identity(),
        model: model.identity(),
        algorithm: model.algorithm(),
        regression_split: report.split_identity(),
        expected_report: report.identity(),
        expected_summary: report.summary(),
    };
    manifest.identity = identity::identity(&manifest);
    Ok(manifest)
}
