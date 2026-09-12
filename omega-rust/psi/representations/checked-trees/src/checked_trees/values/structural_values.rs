//! Fresh structural values and their selected scalar dispatch paths.

use crate::{CheckedOperatorUseFact, CheckedScalarComputationHandle, CheckedScalarDispatchPattern};
use arena::Arena;
use arena::{Handle, HandleSpan};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, TableMatchArm};
use typed_trees::types::TypeReferenceHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralValuePlans {
    pub roots: Arena<CheckedStructuralValueRoot>,
    pub nodes: Arena<CheckedStructuralValue>,
    pub dispatch_arms: Arena<CheckedStructuralDispatchArm>,
}

pub type CheckedStructuralValueHandle = Handle<CheckedStructuralValue>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralValueRoot {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_ordinal: u32,
    pub expression: ExpressionHandle,
    pub type_reference: TypeReferenceHandle,
    pub root: CheckedStructuralValueHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralValue {
    pub expression: ExpressionHandle,
    pub kind: CheckedStructuralValueKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedStructuralValueKind {
    Case(crate::CheckedScalarCaseConstruction),
    Dispatch {
        subject: CheckedScalarComputationHandle,
        arms: HandleSpan<CheckedStructuralDispatchArm>,
    },
}

impl Default for CheckedStructuralValueKind {
    fn default() -> Self {
        Self::Case(crate::CheckedScalarCaseConstruction::default())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralDispatchArm {
    pub source_arm: Handle<TableMatchArm>,
    pub equality_use: Handle<CheckedOperatorUseFact>,
    pub pattern: CheckedScalarDispatchPattern,
    pub value: CheckedStructuralValueHandle,
}

impl CheckedStructuralValuePlans {
    pub fn root_at(
        &self,
        state: SymbolHandle,
        statement_ordinal: u32,
    ) -> Option<&CheckedStructuralValueRoot> {
        let mut roots = self
            .roots
            .iter()
            .map(|(_, root)| root)
            .filter(|root| root.state == state && root.statement_ordinal == statement_ordinal);
        let root = roots.next()?;
        roots.next().is_none().then_some(root)
    }
}
