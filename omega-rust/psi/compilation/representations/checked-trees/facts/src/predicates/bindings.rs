//! A true checked formula and its exact current storage observations.
//! Expression positions remain in the originating invocation namespace;
//! bindings, not a later state's parameter ordering, identify current places.
use crate::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedStructuralCaseSubject, Place,
};
use arena::{Handle, HandleSpan};
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceHandle;

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct BooleanPredicate {
    pub role: BooleanPredicateRole,
    pub machine: SymbolHandle,
    pub entry_state: SymbolHandle,
    pub expression: CheckedBooleanExpression,
    pub bindings: HandleSpan<BooleanPredicateBinding>,
}

impl Default for BooleanPredicate {
    fn default() -> Self {
        Self {
            role: BooleanPredicateRole::CurrentObservation,
            machine: SymbolHandle::invalid(),
            entry_state: SymbolHandle::invalid(),
            expression: CheckedBooleanExpression::Constant(false),
            bindings: HandleSpan::empty(),
        }
    }
}

/// Exact observations in one retained Boolean formula. A sum subject is a
/// typed structural value, never a fabricated scalar discriminant. The case
/// tested remains on its StructuralCaseMembership atom in the formula.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum BooleanPredicateOperand {
    Scalar(CheckedScalarExpression),
    CaseSubject(CheckedStructuralCaseSubject),
}

impl BooleanPredicateOperand {
    pub fn scalar(&self) -> Option<&CheckedScalarExpression> {
        match self {
            Self::Scalar(value) => Some(value),
            Self::CaseSubject(_) => None,
        }
    }

    pub fn primitive_type(&self) -> Option<typed_trees::types::PrimitiveType> {
        self.scalar()?.primitive_type()
    }
}

impl From<CheckedScalarExpression> for BooleanPredicateOperand {
    fn from(value: CheckedScalarExpression) -> Self {
        Self::Scalar(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct BooleanPredicateBinding {
    /// One genuine observation in the originating invocation namespace.
    /// Its typed place and epoch remain independently authenticated.
    pub operand: BooleanPredicateOperand,
    pub place: Handle<Place>,
    pub type_reference: TypeReferenceHandle,
}

impl Default for BooleanPredicateBinding {
    fn default() -> Self {
        Self {
            operand: BooleanPredicateOperand::Scalar(CheckedScalarExpression::Boolean(Box::new(
                CheckedBooleanExpression::Constant(false),
            ))),
            place: Handle::invalid(),
            type_reference: TypeReferenceHandle::invalid(),
        }
    }
}

/// Historical entry conditions describe control reachability, never current
/// storage. An untransportable guard explicitly blocks entry-condition proof.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum BooleanPredicateRole {
    #[default]
    CurrentObservation,
    EntryPathCondition,
    UnknownEntryPathCondition,
}
