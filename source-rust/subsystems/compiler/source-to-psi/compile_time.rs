//! Source inputs and receipts used while resolving and preparing a program.
pub(crate) use crate::product::source_targets::FixedLayoutTargetScope;
pub(crate) use build_time_values::BuildTimeValue;
pub use declaration_scope::DeclarationSelectionAuthority as BuildTimeSelectionAuthority;
pub use source_completion_accounting::{PureEvaluationRecord, SourceCompletionAccounting};
pub(crate) use source_completion_accounting::{
    PureTerminalEvaluationOutcome, PureTerminalEvaluationReceipt,
};
pub use source_symbol_table::source_inputs::InvocationSourceConsent;
mod fixed_layouts;
pub(crate) use fixed_layouts::evaluate_fixed_layout_applications;
mod frontier;
pub(crate) use frontier::PreCheckEvaluation;

mod demands;
pub(crate) use demands::demanded_declarations;

mod constants;
pub(crate) use constants::{
    evaluate_const_arguments, evaluate_constant_initializers_with_source_consent,
    validate_constant_evaluations,
};

mod calls;
pub(crate) use calls::{
    InvocationTarget, MeasuredEvaluation, PreparedBuildMachineEntry, PreparedBuildMachineProgram,
};

pub(crate) use calls::invocation_error;
