use omega_symbol_resolved_trees::expression::NamePath;
use omega_symbol_resolved_trees::name::DiagnosticName;
use omega_syntax_trees as syntax;

pub(crate) fn lower_name(name: &syntax::identifier::Identifier) -> DiagnosticName {
    DiagnosticName::new(name.as_str(), name.source_span())
}

pub(crate) fn lower_name_members<'name>(
    members: impl IntoIterator<Item = &'name syntax::identifier::Identifier>,
) -> NamePath {
    NamePath::unresolved_from_iter(members.into_iter().map(lower_name))
}
