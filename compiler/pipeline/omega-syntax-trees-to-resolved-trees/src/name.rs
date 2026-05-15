use omega_resolved_trees::expression::NamePath;
use omega_resolved_trees::name::DiagnosticName;
use omega_syntax_trees as syntax;

pub(crate) fn lower_name(name: &syntax::identifier::Identifier) -> DiagnosticName {
    DiagnosticName::generated(name.as_str())
}

pub(crate) fn lower_name_members<'name>(
    members: impl IntoIterator<Item = &'name syntax::identifier::Identifier>,
) -> NamePath {
    NamePath::unresolved(members.into_iter().map(lower_name).collect())
}
