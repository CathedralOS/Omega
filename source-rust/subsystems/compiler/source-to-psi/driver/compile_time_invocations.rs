//! Evaluate demanded source values. Layouts, access plans and constants select
//! observations and operands. Nested invocations use the same typed caller,
//! checking path and interpreter, then return their value to the waiting stage.
pub(crate) use crate::compile_product::source_targets::FixedLayoutTargetScope;
pub(crate) use build_model::values::BuildTimeValue;
pub use source_completion_accounting::{PureEvaluationRecord, SourceCompletionAccounting};
pub(crate) use source_completion_accounting::{
    PureTerminalEvaluationOutcome, PureTerminalEvaluationReceipt,
};
pub use source_symbol_table::source_inputs::InvocationSourceConsent;
mod demand_order;
mod demanded_values;
#[cfg(test)]
pub(crate) use demanded_values::EvaluatedValues;
pub(crate) use demanded_values::{ValueDemand, evaluate_demanded_values};
mod domain_access_plans;
mod fixed_layouts;
mod frontier;
pub(crate) use frontier::PreCheckEvaluation;

mod demands;
pub(crate) use demands::demanded_declarations;

mod constants;
pub(crate) use constants::validate_constant_evaluations;

mod calls;
mod typed_invocation;
pub(crate) use calls::{InvocationTarget, PreparedBuildMachineEntry, PreparedBuildMachineProgram};

pub(crate) use calls::invocation_error;
