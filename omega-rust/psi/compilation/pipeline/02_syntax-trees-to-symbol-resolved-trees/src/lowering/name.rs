//! Identifier to diagnostic name.

use symbol_resolved_trees::name::DiagnosticName;
use syntax_trees::{self as syntax, SyntaxTrees};

use crate::resolution::lowerer::Lowerer;
use arena::HandleSpan;

pub(crate) fn lower_name(name: &syntax::identifier::Identifier) -> DiagnosticName {
    DiagnosticName::from_str(name.as_str(), name.source_span())
}

pub(crate) fn lower_operator_path_name(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    name: HandleSpan<syntax::identifier::Identifier>,
) -> HandleSpan<DiagnosticName> {
    let mut span = HandleSpan::empty();

    for member in syntax_trees.items.identifier_path_members(name) {
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .operator_path_members
            .append_to_span(&mut span, lower_name(member));
    }

    span
}
