use crate::symbol_resolved_trees::expression::ExpressionHandle;
use crate::symbol_resolved_trees::name::DiagnosticName;
use crate::symbol_resolved_trees::signature::StateParameter;
use crate::symbol_resolved_trees::types::TypeReference;
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
