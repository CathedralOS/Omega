use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionTable;
use syntax_trees::item::ItemTable;
use syntax_trees::statement::StatementTable;
use syntax_trees::types::TypeReferenceTable;

pub fn exprs(syntax_trees: &mut SyntaxTrees) -> &mut ExpressionTable {
    &mut syntax_trees.expressions
}

pub fn exprs_of(syntax_trees: &SyntaxTrees) -> &ExpressionTable {
    &syntax_trees.expressions
}

pub fn items(syntax_trees: &mut SyntaxTrees) -> &mut ItemTable {
    &mut syntax_trees.items
}

pub fn items_of(syntax_trees: &SyntaxTrees) -> &ItemTable {
    &syntax_trees.items
}

pub fn stmts(syntax_trees: &mut SyntaxTrees) -> &mut StatementTable {
    &mut syntax_trees.statements
}

pub fn stmts_of(syntax_trees: &SyntaxTrees) -> &StatementTable {
    &syntax_trees.statements
}

pub fn type_refs(syntax_trees: &mut SyntaxTrees) -> &mut TypeReferenceTable {
    &mut syntax_trees.type_references
}

pub fn type_refs_of(syntax_trees: &SyntaxTrees) -> &TypeReferenceTable {
    &syntax_trees.type_references
}
