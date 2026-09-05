use omega_optimization_core::{
    ExternalCandidateFeatures, ExternalDecisionContext, ExternalDecisionPoint,
};
use sha2::{Digest, Sha256};

use super::model::CapturedLog;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DecisionSurfaceIdentity([u8; 32]);

impl DecisionSurfaceIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OfflinePolicyCorpusIdentity([u8; 32]);

impl OfflinePolicyCorpusIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

pub fn decision_surface_identity(
    context: ExternalDecisionContext,
    point: &ExternalDecisionPoint,
) -> DecisionSurfaceIdentity {
    let mut digest = Sha256::new();
    digest.update(b"omega.offline-policy-decision-surface.sha256.v1\0");
    encode_context(&mut digest, context);
    digest.update(point.input().bytes());
    digest.update(point.rule().bytes());
    digest.update((point.legal_candidates().len() as u64).to_le_bytes());
    for candidate in point.legal_candidates() {
        encode_candidate(&mut digest, candidate);
    }
    DecisionSurfaceIdentity(digest.finalize().into())
}

pub(super) fn corpus_identity(logs: &[CapturedLog]) -> OfflinePolicyCorpusIdentity {
    let mut digest = Sha256::new();
    digest.update(b"omega.offline-policy-corpus.sha256.v1\0");
    digest.update((logs.len() as u64).to_le_bytes());
    for log in logs {
        digest.update([log.split.tag()]);
        digest.update((log.encoded.len() as u64).to_le_bytes());
        digest.update(&log.encoded);
    }
    OfflinePolicyCorpusIdentity(digest.finalize().into())
}

fn encode_context(digest: &mut Sha256, context: ExternalDecisionContext) {
    digest.update(context.schema().bytes());
    digest.update(context.source().bytes());
    digest.update(context.selections().bytes());
    digest.update(context.phase_selections().bytes());
    digest.update(context.target().bytes());
    digest.update(context.rule_set().bytes());
    digest.update(context.cost_model().bytes());
}

fn encode_candidate(digest: &mut Sha256, candidate: &ExternalCandidateFeatures) {
    digest.update(candidate.candidate().bytes());
    digest.update(candidate.predicted_cost_delta().to_le_bytes());
    digest.update(candidate.consumed_analyses().encode());
    digest.update((candidate.consumed_facts().len() as u64).to_le_bytes());
    for fact in candidate.consumed_facts() {
        digest.update(fact.encode());
    }
}
