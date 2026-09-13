#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Offline custody for recorded policy data.
//!
//! This tooling crate admits canonical external-policy logs for offline use. It
//! has no optimizer, compiler, process, sandbox, or build-selection authority.
//! Start in `cost_threshold_policy.rs` for the train/evaluate lifecycle and its
//! independent replay checks; `corpus` owns admission of recorded inputs.

mod corpus;
mod cost_threshold_policy;

pub use corpus::{
    DecisionSurfaceIdentity, OfflinePolicyCorpusError, OfflinePolicyCorpusIdentity,
    OfflinePolicyCorpusReceipt, OfflinePolicyDecisionExample, OfflinePolicySplit,
    ValidatedOfflinePolicyCorpus, admit_external_decision_logs, decision_surface_identity,
    decode_offline_policy_corpus, split_for_source,
};
pub use cost_threshold_policy::{
    CostThresholdV1Model, OfflinePolicyAlgorithmIdentity, OfflinePolicyConfusion,
    OfflinePolicyEvaluationReport, OfflinePolicyEvaluationSummary, OfflinePolicyModelIdentity,
    OfflinePolicyPrediction, OfflinePolicyReferenceError, OfflinePolicyRegressionManifest,
    OfflinePolicyRegressionManifestIdentity, OfflinePolicyReportIdentity,
    OfflinePolicySplitIdentity, cost_threshold_v1_algorithm_identity,
    create_cost_threshold_v1_regression_manifest, decode_cost_threshold_v1_model,
    decode_cost_threshold_v1_regression_manifest, decode_cost_threshold_v1_report,
    evaluate_cost_threshold_v1, offline_policy_split_identity, train_cost_threshold_v1,
};
