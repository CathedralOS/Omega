//! Canonical optimization decision records; no executable candidate policy.

mod baseline;
mod external_schema;

pub use baseline::{
    BaselineDecisionLog, BaselineDecisionLogBuilder, BaselineDecisionLogDecodeError,
    BaselineDecisionOutcome, BaselineDecisionRecord, BaselineDecisionRecordError,
    ValidatedCandidateSummary,
};
pub use external_schema::{
    ExternalCandidateFeatures, ExternalDecisionAction, ExternalDecisionContext,
    ExternalDecisionLog, ExternalDecisionPoint, ExternalDecisionSchemaError,
    external_psi_decision_schema_v2_identity, psi_target_neutral_decision_target_v2_identity,
};
