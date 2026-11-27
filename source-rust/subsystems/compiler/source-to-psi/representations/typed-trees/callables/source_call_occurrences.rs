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
