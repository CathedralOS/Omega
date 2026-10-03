//! A true checked formula and its exact current storage observations.
//! Expression positions remain in the originating invocation namespace;
//! bindings, not a later state's parameter ordering, identify current places.
use crate::{CheckedBooleanExpression, CheckedScalarExpression, Place};
use arena::{Handle, HandleSpan};
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BooleanPredicate {
    pub machine: SymbolHandle,
    pub entry_state: SymbolHandle,
    pub expression: CheckedBooleanExpression,
    pub bindings: HandleSpan<BooleanPredicateBinding>,
}

impl Default for BooleanPredicate {
    fn default() -> Self {
        Self {
            machine: SymbolHandle::invalid(),
            entry_state: SymbolHandle::invalid(),
            expression: CheckedBooleanExpression::Constant(false),
            bindings: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BooleanPredicateBinding {
    /// One existing scalar observation in the formula, including its original
    /// dense scalar or authored structural parameter position and field path.
    pub operand: CheckedScalarExpression,
    pub place: Handle<Place>,
    pub type_reference: TypeReferenceHandle,
}

impl Default for BooleanPredicateBinding {
    fn default() -> Self {
        Self {
            operand: CheckedScalarExpression::Boolean(Box::new(
                CheckedBooleanExpression::Constant(false),
            )),
            place: Handle::invalid(),
            type_reference: TypeReferenceHandle::invalid(),
        }
    }
}
