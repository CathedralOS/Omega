/*
Checked-Psi dispatch settlement for exact build-selected realizations.

We start at provider_binding.rs for complete source-dependent preparation;
selected_execution.rs is its retained result rather than another phase. The
optional checked-tree oracle prepares its own bindings beside its only
consumer in the driver's corpus runner.
*/

/*
Psi owns source checking and execution plans. We own realization associations
and target-selected provider binding without reopening source checking.
*/

#![forbid(unsafe_code)]

mod compiler_intrinsic;
mod intrinsic_review;
mod numeric_provider;
pub use numeric_provider::NumericProviderSelection;
mod provider_binding;
mod selected_dispatch;
mod selected_execution;

pub use compiler_intrinsic::{
    SelectedCompilerIntrinsicExecutionIdentity,
    derive_selected_compiler_intrinsic_execution_identity_for_row,
};
pub(crate) use intrinsic_review::{
    ResolvedAcceptedSemanticBinding, resolve_accepted_service_binding,
    retain_selected_compiler_intrinsic_review_identities,
};
pub use provider_binding::{
    PreparedBindingDeclarations, ProviderBindingRequest, prepare_selected_execution,
};
pub use selected_execution::SelectedExecution;

pub use selected_dispatch::{
    CheckedOperatorAuthoredUseKind, derive_checked_nongeneric_operator_application_realizations,
    derive_checked_specialized_operator_application_realizations,
    derive_selected_primitive_float_binary_execution, selected_float_comparison_executions,
    settle_selected_comparison_execution,
};
