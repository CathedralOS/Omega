#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Offline custody for recorded policy data.
//!
//! This tooling crate admits canonical external-policy logs for offline use. It
//! has no optimizer, compiler, process, sandbox, or build-selection authority.

mod corpus;

pub use corpus::{
    DecisionSurfaceIdentity, OfflinePolicyCorpusError, OfflinePolicyCorpusIdentity,
    OfflinePolicyCorpusReceipt, OfflinePolicyDecisionExample, OfflinePolicySplit,
    ValidatedOfflinePolicyCorpus, admit_external_decision_logs, decision_surface_identity,
    decode_offline_policy_corpus, split_for_source,
};
