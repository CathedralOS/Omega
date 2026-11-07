//! Result-less completion expressions retain selection and invocation custody.
//! Unit creates no scalar slot or structural home. Subjects and patterns use
//! the same evaluated dispatch operands as value-producing selections.

use arena::{Arena, Handle, HandleSpan};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, TableMatchArm};

pub type CheckedCompletionExpressionHandle = Handle<CheckedCompletionExpression>;

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedCompletionPlans {
    pub nodes: Arena<CheckedCompletionExpression>,
    pub arms: Arena<CheckedCompletionArm>,
    pub roots: Vec<CheckedCompletionRoot>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedCompletionRoot {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_ordinal: u32,
    pub expression: ExpressionHandle,
    pub root: CheckedCompletionExpressionHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedCompletionExpression {
    pub expression: ExpressionHandle,
    pub normal_continuation: bool,
    pub kind: CheckedCompletionExpressionKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum CheckedCompletionExpressionKind {
    #[default]
    Missing,
    Invocation {
        source_call: Handle<crate::FlowCallFact>,
    },
    Dispatch {
        subject: super::CheckedDispatchSubject,
        arms: HandleSpan<CheckedCompletionArm>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedCompletionArm {
    pub source_arm: Handle<TableMatchArm>,
    pub equality_use: Handle<crate::CheckedOperatorUseFact>,
    pub pattern: super::CheckedScalarDispatchPattern,
    pub completion: CheckedCompletionExpressionHandle,
    pub comparison: CheckedCompletionComparison,
}

impl CheckedCompletionPlans {
    pub fn seal(&mut self) {
        self.roots
            .sort_unstable_by_key(CheckedCompletionRoot::coordinate);
    }

    pub fn root_at(
        &self,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement: u32,
        expression: ExpressionHandle,
    ) -> Option<&CheckedCompletionRoot> {
        let key = (
            machine.arena_index(),
            machine.generation(),
            state.arena_index(),
            state.generation(),
            statement,
            expression.arena_index(),
            expression.generation(),
        );
        let first = self.roots.partition_point(|root| root.coordinate() < key);
        let root = self
            .roots
            .get(first)
            .filter(|root| root.coordinate() == key)?;
        (!self
            .roots
            .get(first + 1)
            .is_some_and(|next| next.coordinate() == key))
        .then_some(root)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum CheckedCompletionComparison {
    #[default]
    Predicate,
    Boolean,
    Integer,
}

impl CheckedCompletionRoot {
    fn coordinate(&self) -> (u32, u32, u32, u32, u32, u32, u32) {
        (
            self.machine.arena_index(),
            self.machine.generation(),
            self.state.arena_index(),
            self.state.generation(),
            self.statement_ordinal,
            self.expression.arena_index(),
            self.expression.generation(),
        )
    }
}
