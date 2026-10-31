//! Crate-local owners for the typed tables every rewrite walks.
//!
//! Elaboration reaches one program's tables through many short chains, and
//! rustfmt breaks each chain across three lines. Naming the receiver once
//! keeps a whole lookup on one line without moving any logic.

use typed_trees::TypedTreeTables;
use typed_trees::expression::ExpressionTable;

/// The expression table of the program under elaboration.
///
/// Takes the table bundle so a bare program, a checking input and a staged
/// copy all reach it through the same call.
pub(crate) fn expressions_of(tables: &TypedTreeTables) -> &ExpressionTable {
    &tables.expression_table
}

/// The expression table of the program under elaboration, for rewrite.
pub(crate) fn expressions_of_mut(tables: &mut TypedTreeTables) -> &mut ExpressionTable {
    &mut tables.expression_table
}
