//! Appending parsed nodes into the caller's `SyntaxTrees` arenas.
//!
//! The grammar does not build linked nodes: every expression, statement, item
//! and type reference is appended to its table and named by a handle, and a run
//! of children is a span of consecutive handles. `tables` gives the grammar its
//! views onto those tables; `handle_spans` stages the span a run of appends
//! returns.

pub(crate) mod handle_spans;
pub(crate) mod tables;
