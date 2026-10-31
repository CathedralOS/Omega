//! Read views onto the carrier's table storages.
//!
//! `program.tables.bodies.expressions` is four links deep, so every read of a
//! body expression — and every read of its statements — is emitted vertically
//! with each link on its own line. These free functions name the table they
//! return: one call root and one link stay on the site's line.
//!
//! They are free functions and not methods for that reason alone. A method
//! would keep the receiver a field chain, which `rustfmt` measures against
//! `chain_width` and splits; a call root is measured against `max_width` and
//! keeps the short chain inline.
//!
//! These are read views. A `&mut` table method — `insert`, `expression_mut`,
//! `set_source_span`, `attach_authored_selection_occurrences`, … — cannot go
//! through one, so write sites keep the raw field.

use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::expression::ExpressionTable;
use symbol_resolved_trees::statement::StatementTable;

/// The expressions of every resolved body.
pub(crate) fn body_expressions(program: &SymbolResolvedTrees) -> &ExpressionTable {
    &program.tables.bodies.expressions
}

/// The statements of every resolved body.
pub(crate) fn body_statements(program: &SymbolResolvedTrees) -> &StatementTable {
    &program.tables.bodies.statements
}
