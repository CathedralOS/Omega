//! Crate-local views onto the syntax tables a parse appends to.
//!
//! `SyntaxTrees` derefs to its tables, so `syntax_trees.expressions.insert(..)` is a
//! field-rooted chain that rustfmt measures against `chain_width` and re-emits one
//! segment per line. Written as a call-rooted chain the same expression is measured
//! against `fn_call_width` and stays on one line, which is why the grammar reaches
//! the tables through these projections instead of through the field.

use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionTable;

/// The expression table this parse appends to.
pub fn exprs(syntax_trees: &mut SyntaxTrees) -> &mut ExpressionTable {
    &mut syntax_trees.expressions
}

/// The expression table this parse reads.
pub fn exprs_of(syntax_trees: &SyntaxTrees) -> &ExpressionTable {
    &syntax_trees.expressions
}
