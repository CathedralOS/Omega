//! The tables this stage reads and writes, named once: a call site spells the
//! table instead of chaining `tables.bodies.expressions` or
//! `typed_trees.expression_table` into every method it calls.
//!
//! Every accessor takes the storage that owns the table (`TypedTreeTables`,
//! `SymbolResolvedTrees`) rather than the wrapper above it, so `&TypedTrees`,
//! `&mut TypedTrees` and `&trees.tables` all reach the same call.

use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::expression::ExpressionTable as ResolvedExpressions;
use symbol_resolved_trees::statement::StatementTable as ResolvedStatements;
use typed_trees::TypedTreeTables;
use typed_trees::expression::ExpressionTable;
use typed_trees::statement::StatementTable;

/// The typed expression table.
pub(crate) fn expression_table(tables: &TypedTreeTables) -> &ExpressionTable {
    &tables.expression_table
}

/// The typed expression table, for the readers that write it.
pub(crate) fn expression_table_mut(tables: &mut TypedTreeTables) -> &mut ExpressionTable {
    &mut tables.expression_table
}

/// The typed statement table, for the readers that write it.
pub(crate) fn statement_table_mut(tables: &mut TypedTreeTables) -> &mut StatementTable {
    &mut tables.statement_table
}

/// The resolved expression table, the input this stage types.
pub(crate) fn resolved_expressions(source: &SymbolResolvedTrees) -> &ResolvedExpressions {
    &source.tables.bodies.expressions
}

/// The resolved statement table, the input this stage types.
pub(crate) fn resolved_statements(source: &SymbolResolvedTrees) -> &ResolvedStatements {
    &source.tables.bodies.statements
}
