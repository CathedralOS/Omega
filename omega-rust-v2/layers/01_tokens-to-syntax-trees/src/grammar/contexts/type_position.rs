//! Whether an arrow belongs to the type or the surrounding expression.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TypePosition {
    /// A parameter, a field, a result: nothing around the type competes for its tokens.
    Plain,

    /// After `as`. Here `->` is left alone, because in an arm written `x as T -> ...` the arrow
    /// belongs to the arm. Everywhere else `A -> B` is a function type.
    Cast,
}
