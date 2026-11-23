//! What may follow a type in its caller's grammar.

use super::expression_context::ExpressionContext;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TypePosition {
    /// A type with no domain qualification or surrounding expression.
    Plain,

    /// A type which may be followed by `in` and a domain path.
    Qualified,
    /// A qualified type after `->`; a header clause can follow it.
    Result,

    /// After `as`. Here `->` is left alone, because in an arm written `x as T -> ...` the arrow
    /// belongs to the arm. Everywhere else `A -> B` is a function type.
    Cast(ExpressionContext),
}
