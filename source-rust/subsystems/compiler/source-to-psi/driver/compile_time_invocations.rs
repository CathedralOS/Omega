//! Evaluate demanded source values. Layouts, access plans and constants select
//! observations and operands. Nested invocations use the same typed caller,
//! checking path and interpreter, then return their value to the waiting stage.
pub(crate) use crate::compile_product::source_targets::FixedLayoutTargetScope;
pub(crate) use build_time_values::BuildTimeValue;
pub use source_completion_accounting::{PureEvaluationRecord, SourceCompletionAccounting};
pub(crate) use source_completion_accounting::{
    PureTerminalEvaluationOutcome, PureTerminalEvaluationReceipt,
};
pub use source_symbol_table::source_inputs::InvocationSourceConsent;
mod domain_access_plans;
pub(crate) use domain_access_plans::evaluate as evaluate_domain_access_plans;
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
mod typed_invocation;
pub(crate) use calls::{InvocationTarget, PreparedBuildMachineEntry, PreparedBuildMachineProgram};

pub(crate) use calls::invocation_error;
