//! Source inputs and receipts used while resolving and preparing a program.
pub use crate::preparation::WireCompatibilityDemand;
pub use crate::source_targets::FixedLayoutTargetScope;
pub use build_scalar_evaluation::{ScalarScopeEvaluation, prepare_scalar_applications};
pub use build_time_values::BuildTimeValue;
pub use declaration_scope::DeclarationSelectionAuthority as BuildTimeSelectionAuthority;
pub use source_completion_accounting::{
    CanonicalScalarEvaluationReceipt, PureEvaluationRecord, PureTerminalEvaluationOutcome,
    PureTerminalEvaluationReceipt, SourceCompletionAccounting,
};
pub use source_symbol_table::source_inputs::InvocationSourceConsent;
#[path = "evaluation/fixed_layouts/mod.rs"]
mod fixed_layouts;
pub(crate) use fixed_layouts::evaluate_fixed_layout_applications;
#[path = "evaluation/frontier.rs"]
mod frontier;
pub(crate) use frontier::PreCheckEvaluation;

#[path = "evaluation/native_bindings.rs"]
pub mod native_bindings;
#[path = "evaluation/product_declarations.rs"]
pub mod product_declarations;
#[path = "evaluation/provider_declarations.rs"]
pub mod provider_declarations;
#[path = "evaluation/retained_product_application.rs"]
pub mod retained_product_application;

use crate::declaration_selection::{
    BuildOperation, ExecutedProductSelection, ProductDeclarationRequest,
};

pub fn build_requests_any(
    declared: &[std::sync::Arc<ProductDeclarationRequest>],
    executed: &[ExecutedProductSelection],
    operations: &[BuildOperation],
) -> bool {
    declared
        .iter()
        .any(|request| operations.contains(&request.operation))
        || executed
            .iter()
            .any(|event| operations.contains(&event.request.operation))
}

#[path = "evaluation/demands.rs"]
mod demands;
pub(crate) use demands::demanded_declarations;

#[path = "evaluation/constants.rs"]
mod constants;
pub use constants::{
    evaluate_const_arguments, evaluate_constant_initializers_with_source_consent, validate_constant_evaluations,
};

#[path = "evaluation/calls.rs"]
mod calls;
pub use calls::{InvocationTarget, MeasuredEvaluation, PreparedBuildMachineEntry, PreparedBuildMachineProgram};

pub(crate) use calls::invocation_error;
