//! Identifier to diagnostic name.

use symbol_resolved_trees::name::DiagnosticName;
use syntax_trees::{self as syntax, SyntaxTrees};

use crate::resolution::lowerer::Lowerer;
use arena::HandleSpan;

pub(crate) fn lower_name(
    syntax_trees: &syntax_trees::SyntaxTrees,
    name: &syntax::identifier::Identifier,
) -> DiagnosticName {
    DiagnosticName::from_str(name.as_str(syntax_trees), name.source_span())
}

pub(crate) fn lower_operator_path_name(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    name: HandleSpan<syntax::identifier::Identifier>,
) -> HandleSpan<DiagnosticName> {
    lower_name_members(
        syntax_trees,
        name,
        &mut lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .operator_path_members,
    )
}

/// Lower an identifier path into a fresh span of `arena`.
pub(crate) fn lower_name_members(
    syntax_trees: &SyntaxTrees,
    name: HandleSpan<syntax::identifier::Identifier>,
    arena: &mut arena::Arena<DiagnosticName>,
) -> HandleSpan<DiagnosticName> {
    let mut span = HandleSpan::empty();
    for member in syntax_trees.items.identifier_path_members(name) {
        arena.append_to_span(&mut span, lower_name(syntax_trees, member));
    }
    span
}
