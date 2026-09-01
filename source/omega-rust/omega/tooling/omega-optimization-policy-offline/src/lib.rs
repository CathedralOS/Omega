#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Offline custody for recorded policy data.
//!
//! This tooling crate admits canonical external-policy logs for offline use. It
//! has no optimizer, compiler, process, sandbox, or build-selection authority.

mod corpus;
mod reference_policy;

pub use corpus::{
    DecisionSurfaceIdentity, OfflinePolicyCorpusError, OfflinePolicyCorpusIdentity,
    OfflinePolicyCorpusReceipt, OfflinePolicyDecisionExample, OfflinePolicySplit,
    ValidatedOfflinePolicyCorpus, admit_external_decision_logs, decision_surface_identity,
    decode_offline_policy_corpus, split_for_source,
};
pub use reference_policy::{
    CostThresholdV1Model, OfflinePolicyAlgorithmIdentity, OfflinePolicyConfusion,
    OfflinePolicyEvaluationReport, OfflinePolicyEvaluationSummary, OfflinePolicyModelIdentity,
    OfflinePolicyPrediction, OfflinePolicyReferenceError, OfflinePolicyReportIdentity,
    OfflinePolicySplitIdentity, cost_threshold_v1_algorithm_identity,
    decode_cost_threshold_v1_model, decode_cost_threshold_v1_report, evaluate_cost_threshold_v1,
    offline_policy_split_identity, train_cost_threshold_v1,
};
