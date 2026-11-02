//! Crate-local views onto the syntax tables a parse appends to.
//!
//! `SyntaxTrees` derefs to its tables, so `syntax_trees.expressions.insert(..)` is a
//! field-rooted chain that rustfmt measures against `chain_width` and re-emits one
//! segment per line. Written as a call-rooted chain the same expression is measured
//! against `fn_call_width` and stays on one line, which is why the grammar reaches
//! the tables through these projections instead of through the field.

use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionTable;
use syntax_trees::item::ItemTable;
use syntax_trees::statement::StatementTable;
use syntax_trees::types::TypeReferenceTable;

/// The expression table this parse appends to.
pub fn exprs(syntax_trees: &mut SyntaxTrees) -> &mut ExpressionTable {
    &mut syntax_trees.expressions
}

/// The expression table this parse reads.
pub fn exprs_of(syntax_trees: &SyntaxTrees) -> &ExpressionTable {
    &syntax_trees.expressions
}

/// The item table this parse appends to.
pub fn items(syntax_trees: &mut SyntaxTrees) -> &mut ItemTable {
    &mut syntax_trees.items
}

/// The item table this parse reads.
pub fn items_of(syntax_trees: &SyntaxTrees) -> &ItemTable {
    &syntax_trees.items
}

/// The statement table this parse appends to.
pub fn stmts(syntax_trees: &mut SyntaxTrees) -> &mut StatementTable {
    &mut syntax_trees.statements
}

/// The statement table this parse reads.
pub fn stmts_of(syntax_trees: &SyntaxTrees) -> &StatementTable {
    &syntax_trees.statements
}

/// The type-reference table this parse appends to.
pub fn type_refs(syntax_trees: &mut SyntaxTrees) -> &mut TypeReferenceTable {
    &mut syntax_trees.type_references
}

/// The type-reference table this parse reads.
pub fn type_refs_of(syntax_trees: &SyntaxTrees) -> &TypeReferenceTable {
    &syntax_trees.type_references
}
