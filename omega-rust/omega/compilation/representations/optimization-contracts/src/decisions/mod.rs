// Canonical optimization decision records (`baseline` and `external_schema`).
//
// Neither module executes candidate selection policy; they record and validate
// the canonical wire and identity representations of already-validated
// candidate sets and chosen actions.

mod baseline;
mod external_schema;

pub use baseline::{
    BaselineDecisionLog, BaselineDecisionLogBuilder, BaselineDecisionLogDecodeError,
    BaselineDecisionOutcome, BaselineDecisionRecordError, ValidatedCandidateSummary,
};
pub use external_schema::{
    ExternalCandidateFeatures, ExternalDecisionAction, ExternalDecisionContext,
    ExternalDecisionLog, ExternalDecisionPoint, ExternalDecisionSchemaError,
    external_psi_decision_schema_identity, psi_target_neutral_decision_target_identity,
};
