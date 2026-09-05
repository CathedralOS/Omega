//! Executable scalar evaluation, separate from pure proposition expressions.

use super::*;
use typed_trees::types::PrimitiveType;

pub type CheckedScalarComputationHandle = Handle<CheckedScalarComputation>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedScalarComputationPlans {
    pub roots: Arena<CheckedScalarComputationRoot>,
    pub nodes: Arena<CheckedScalarComputation>,
    pub operands: Arena<CheckedScalarComputationHandle>,
}

impl CheckedScalarComputationPlans {
    pub fn root_at(
        &self,
        state: SymbolHandle,
        statement_ordinal: u32,
        role: CheckedScalarExpressionRole,
    ) -> Option<&CheckedScalarComputationRoot> {
        let mut roots = self.roots.iter().map(|(_, root)| root).filter(|root| {
            root.state == state && root.statement_ordinal == statement_ordinal && root.role == role
        });
        let root = roots.next()?;
        roots.next().is_none().then_some(root)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedScalarComputationRoot {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_ordinal: u32,
    pub role: CheckedScalarExpressionRole,
    pub root: CheckedScalarComputationHandle,
}

impl Default for CheckedScalarComputationRoot {
    fn default() -> Self {
        Self {
            machine: SymbolHandle::invalid(),
            state: SymbolHandle::invalid(),
            statement_ordinal: 0,
            role: CheckedScalarExpressionRole::Return,
            root: Handle::invalid(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedScalarComputation {
    /// Exact authored outer expression when this node is a destination root.
    /// Intermediate nodes use the zero handle; source spans are not identity.
    pub authored_root: typed_trees::expression::ExpressionHandle,
    pub primitive_type: PrimitiveType,
    pub kind: CheckedScalarComputationKind,
}

impl Default for CheckedScalarComputation {
    fn default() -> Self {
        Self {
            authored_root: Handle::invalid(),
            primitive_type: PrimitiveType::Bool,
            kind: CheckedScalarComputationKind::Value(CheckedScalarExpression::Boolean(Box::new(
                CheckedBooleanExpression::Constant(false),
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedScalarComputationKind {
    /// Pure source expression in the enclosing state's scalar namespace.
    Value(CheckedScalarExpression),
    Call {
        source_call: Handle<crate::FlowCallFact>,
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
        /// Authored occurrence identity, not its position in execution order.
        call_ordinal: u32,
        arguments: HandleSpan<CheckedScalarComputationHandle>,
    },
    Select {
        condition: CheckedScalarComputationHandle,
        when_true: CheckedScalarComputationHandle,
        when_false: CheckedScalarComputationHandle,
    },
    /// Evaluate operands left-to-right once, then apply a pure template whose
    /// Parameter positions name only these computed operands, not source locals.
    Apply {
        expression: CheckedScalarExpression,
        operands: HandleSpan<CheckedScalarComputationHandle>,
    },
}
