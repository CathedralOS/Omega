//! Build-time execution consumes actual canonical artifacts. The removed
//! source/checked-program evaluators cannot supply acceptance or replay facts.
//! Unsupported source requests stop before effects or result materialization.
#![forbid(unsafe_code)]

mod evaluation_accounting;
mod evaluation_requests;
mod product;
mod scalar_applications;
mod source_frontier;
mod source_jobs;

pub use build_time_values::BuildTimeValue;
pub use evaluation_accounting::{
    CanonicalScalarEvaluationReceipt, MeasuredEvaluation, PureEvaluationRecord,
    PureTerminalEvaluationOutcome, PureTerminalEvaluationReceipt, RecordedPreCheck,
    SourceCompletionAccounting,
};
pub use evaluation_requests::{
    BuildMachineEvaluationError, BuildTimeOperationEvaluation, CURRENT_EVALUATION_SEMANTICS,
    EvaluationSemanticsIdentity, PrivateLayoutPlacementReceipt, SelectedBuildTimeMachineBody,
    UnavailableStaticInput,
};
pub use declaration_contracts::wire::WireCompatibilityDemand;
pub use product::{
    assumed_bindings, build_requests_any, native_bindings, product_declarations, product_policy,
    provider_declarations, test_groups,
};
pub use scalar_applications::{
    AdmittedImportRealizations, InvocationTarget, ScalarApplicationCompletion,
    ScalarScopeEvaluation, TargetImportRealizations, complete_invocation_scalar_applications,
    complete_scalar_applications, evaluate_scalar_applications,
    evaluate_scalar_applications_with_address_model, prepare_scalar_applications,
};
pub use source_frontier::{
    AuthorizedSelection, BuildTimeEvaluationRequest, BuildTimeInvocationCustody,
    BuildTimeSelectionAuthority, BuildTimeSources, InvocationSourceConsent, PreCheckEvaluation,
    PreResolutionEvaluation, evaluate_pre_resolution,
};
pub use source_jobs::*;

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "../../../../../../../tests/support/frontend.rs"]
mod frontend_fixture;
