use crate::types::TypeReferenceHandle;
use arena::HandleSpan;
use symbols::SymbolHandle;

/// A well-founded termination measure declared with the `measure` keyword.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasureDefinition {
    pub symbol: SymbolHandle,
    /// Fully-qualified declaration path, e.g. `Card::PowerOrder`.
    pub name: HandleSpan<crate::name::Identifier>,
    /// The single measured parameter, absent for the lexicographic form.
    pub parameter: Option<crate::signature::StateParameter>,
    /// Well-founded domain type (`usize`).
    pub return_type: TypeReferenceHandle,
    /// `true` for the `lexicographic { .. }` body form.
    pub lexicographic: bool,
    /// Component body expressions, left-to-right.
    pub body: HandleSpan<crate::expression::ExpressionHandle>,
}

impl Default for MeasureDefinition {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: HandleSpan::empty(),
            parameter: None,
            return_type: TypeReferenceHandle::invalid(),
            lexicographic: false,
            body: HandleSpan::empty(),
        }
    }
}
