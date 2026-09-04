use psi_symbol_resolved_trees::name::DiagnosticName;
use psi_syntax_trees as syntax;

pub(crate) fn lower_name(name: &syntax::identifier::Identifier) -> DiagnosticName {
    DiagnosticName::new(name.as_str(), name.source_span())
}
