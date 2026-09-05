use crate::expression::ExpressionHandle;
use crate::name::DiagnosticName;
use crate::signature::StateParameter;
use crate::types::TypeReference;
use arena::HandleSpan;
use symbols::SymbolHandle;

/// A well-founded termination measure declared with the `measure` keyword.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MeasureDefinition {
    pub symbol: SymbolHandle,
    /// Fully-qualified declaration path, e.g. `Card::PowerOrder`.
    pub name: HandleSpan<DiagnosticName>,
    /// The single measured parameter, absent for the lexicographic form.
    pub parameter: Option<StateParameter>,
    /// Well-founded domain type (`usize`).
    pub return_type: Option<TypeReference>,
    /// `true` for the `lexicographic { .. }` body form.
    pub lexicographic: bool,
    /// Component body expressions, left-to-right.
    pub body: HandleSpan<ExpressionHandle>,
}
