use psi_arena::{Arena, Handle};
use psi_symbols::SymbolHandle;
use psi_typed_trees::expression::ExpressionHandle;
use psi_typed_trees::types::TypeReferenceHandle;

pub type CheckedValueHandle = Handle<CheckedValueFact>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedValueOrigin {
    MachineDecrease {
        machine_symbol: SymbolHandle,
        ordinal: usize,
    },
    MachineOwnedDataInitializer {
        machine_symbol: SymbolHandle,
        data_symbol: SymbolHandle,
    },
    StateStatement {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        role: CheckedValueStatementRole,
    },
    NestedExpression {
        parent: ExpressionHandle,
    },
}

impl Default for CheckedValueOrigin {
    fn default() -> Self {
        Self::NestedExpression {
            parent: ExpressionHandle::invalid(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CheckedValueStatementRole {
    #[default]
    Expression,
    AssignmentTargetSubexpression,
    AssignmentValue,
    CallArgument,
    LocalInitializer,
    TransitionGuard,
    TransitionTargetArgument,
    TransitionTargetValue,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedValueFact {
    pub expression: ExpressionHandle,
    pub origin: CheckedValueOrigin,
    /// The checker-resolved declared type of this value at its exact use site.
    /// Invalid means the expression has no standalone declared type (for
    /// example, an anonymous literal) or resolution conservatively failed.
    /// Later lowering may consume validated declaration facts through this
    /// handle; it must never reconstruct a stronger type from storage shape.
    pub type_reference: TypeReferenceHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedValueFacts {
    pub values: Arena<CheckedValueFact>,
}

impl CheckedValueFacts {
    pub fn with_roots(values: Arena<CheckedValueFact>) -> Self {
        Self { values }
    }

    pub fn expression_values(
        &self,
        expression: ExpressionHandle,
    ) -> impl Iterator<Item = (CheckedValueHandle, &CheckedValueFact)> + '_ {
        self.values
            .iter()
            .filter(move |(_, value)| value.expression == expression)
    }
}
