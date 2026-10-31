//! Structured outcomes of the existing pure machine evaluator.
//!
//! Missing static input is unavailable information, never a value or a chosen
//! branch. Ordinary execution and every consumer requiring a completed value
//! still reject it; source preparation may retain that exact obligation.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum UnavailableStaticInput {
    Binding {
        symbol: symbols::SymbolHandle,
        display: String,
    },
    TypePredicate {
        expression: typed_trees::expression::ExpressionHandle,
        input: symbols::SymbolHandle,
    },
}

impl fmt::Display for UnavailableStaticInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Binding { display, .. } => write!(
                formatter,
                "static operand `{display}` has no bound local or parameter"
            ),
            Self::TypePredicate { .. } => {
                formatter.write_str("static type predicate remains unbound during execution")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum BuildMachineEvaluationError {
    StaticInputUnavailable(UnavailableStaticInput),
    Rejected(String),
}

impl fmt::Display for BuildMachineEvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaticInputUnavailable(input) => input.fmt(formatter),
            Self::Rejected(reason) => formatter.write_str(reason),
        }
    }
}

impl std::error::Error for BuildMachineEvaluationError {}

impl From<String> for BuildMachineEvaluationError {
    fn from(reason: String) -> Self {
        Self::Rejected(reason)
    }
}

impl From<BuildMachineEvaluationError> for String {
    fn from(error: BuildMachineEvaluationError) -> Self {
        error.to_string()
    }
}
