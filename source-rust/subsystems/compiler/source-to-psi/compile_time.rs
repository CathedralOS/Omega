//! Source inputs and receipts used while resolving and preparing a program.
pub use crate::preparation::WireCompatibilityDemand;
pub use crate::product::source_targets::FixedLayoutTargetScope;
pub use build_scalar_evaluation::{ScalarScopeEvaluation, prepare_scalar_applications};
pub use build_time_values::BuildTimeValue;
pub use declaration_scope::DeclarationSelectionAuthority as BuildTimeSelectionAuthority;
pub use source_completion_accounting::{
    CanonicalScalarEvaluationReceipt, PureEvaluationRecord, PureTerminalEvaluationOutcome,
    PureTerminalEvaluationReceipt, SourceCompletionAccounting,
};
pub use source_symbol_table::source_inputs::InvocationSourceConsent;
mod fixed_layouts;
pub(crate) use fixed_layouts::evaluate_fixed_layout_applications;
mod frontier;
pub(crate) use frontier::PreCheckEvaluation;

mod demands;
pub(crate) use demands::demanded_declarations;

mod constants;
pub use constants::{
    evaluate_const_arguments, evaluate_constant_initializers_with_source_consent, validate_constant_evaluations,
};

mod calls;
pub use calls::{InvocationTarget, MeasuredEvaluation, PreparedBuildMachineEntry, PreparedBuildMachineProgram};

pub(crate) use calls::invocation_error;
