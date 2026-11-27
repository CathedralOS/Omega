//! Execute the scalar observations requested for a published Psi program against its sealed target inputs.
mod execution;
mod receipts;
mod scopes;

pub use execution::{
    AdmittedImportRealizations, TargetImportRealizations, evaluate_scalar_applications,
};
pub use receipts::{
    CanonicalScalarEvaluationReceipt, PURE_PSI_WORK_BUDGET, PureTerminalEvaluationOutcome,
};
pub use scopes::{ScalarApplicationCompletion, ScalarScopeEvaluation, prepare_scalar_applications};

#[cfg(test)]
mod tests;
