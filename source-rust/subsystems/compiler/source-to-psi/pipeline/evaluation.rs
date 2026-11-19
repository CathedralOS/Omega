//! Source inputs and receipts used while resolving and preparing a program.
pub use crate::preparation::WireCompatibilityDemand;
pub use build_scalar_evaluation::{ScalarScopeEvaluation, prepare_scalar_applications};
pub use build_time_values::BuildTimeValue;
pub use declaration_scope::DeclarationSelectionAuthority as BuildTimeSelectionAuthority;
pub use source_completion_accounting::{
    CanonicalScalarEvaluationReceipt, PureEvaluationRecord, PureTerminalEvaluationOutcome,
    PureTerminalEvaluationReceipt, SourceCompletionAccounting,
};
pub use source_symbol_table::source_inputs::InvocationSourceConsent;
#[path = "evaluation/frontier.rs"]
mod frontier;
pub(crate) use frontier::PreCheckEvaluation;
