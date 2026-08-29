#![forbid(unsafe_code)]

//! Deterministic decision policy over independently validated candidates.
//!
//! `baseline` owns the built-in model-free chooser and its replayable log.
//! `external_schema` owns the versioned record-only extension boundary. Neither
//! path can validate, apply, or invent a rewrite: callers provide only the
//! identities of candidates already accepted by the optimizer.

mod baseline;
mod external_schema;

pub use baseline::{
    BaselineDecisionLog, BaselineDecisionLogDecodeError, BaselineDecisionOutcome,
    BaselineDecisionRecord, BaselineDecisionRecordError, BaselinePolicy, ValidatedCandidateSummary,
};
pub use external_schema::{
    ExternalCandidateFeatures, ExternalDecisionAction, ExternalDecisionContext,
    ExternalDecisionLog, ExternalDecisionPoint, ExternalDecisionSchemaError,
    external_psi_decision_schema_v2_identity, psi_target_neutral_decision_target_v2_identity,
};
