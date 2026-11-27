//! Ordered authored call occurrences for source preparation. This traversal
//! records occurrences and conditional ancestry; it never proves a call.

use crate::source_state::{
    TypedTrees,
    expression::{ExpressionHandle, TableCallExpression},
};

pub trait ExpressionCalls<'program>: Sized {
    type Site;
    fn source(&self) -> &'program TypedTrees;
    fn named_call_is_selected(
        &self,
        expression: ExpressionHandle,
        call: &'program TableCallExpression,
    ) -> bool;
    fn visit_call(
        &mut self,
        expression: ExpressionHandle,
        call: &'program TableCallExpression,
    ) -> Option<Self::Site>;
    fn selected(
        &mut self,
        visit: impl FnOnce(&mut Self) -> Option<Self::Site>,
    ) -> Option<Self::Site>;
}

pub trait StatementCalls<'program>: ExpressionCalls<'program> {
    fn named_statement_call_is_selected(
        &self,
        call: &'program crate::source_state::statement::TableCall,
    ) -> bool;
    fn visit_statement_call(
        &mut self,
        call: &'program crate::source_state::statement::TableCall,
    ) -> Option<Self::Site>;
    fn visit_transition_call(
        &mut self,
        target: &'program crate::source_state::statement::TransitionTargetNode,
    ) -> Option<Self::Site>;
    fn selected_target(
        &mut self,
        target: crate::source_state::statement::TransitionTargetHandle,
        visit: impl FnOnce(&mut Self) -> Option<Self::Site>,
    ) -> Option<Self::Site>;
}
