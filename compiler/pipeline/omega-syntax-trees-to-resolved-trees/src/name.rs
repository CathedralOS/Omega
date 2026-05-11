use omega_syntax_trees as syntax;
use omega_resolved_trees::expression::NamePath;
use omega_resolved_trees::name::ProgramName;

pub(crate) fn lower_name(name: &syntax::identifier::Identifier) -> ProgramName {
    ProgramName::generated(name.as_str())
}

pub(crate) fn lower_name_path(path: &syntax::identifier::IdentifierPath) -> NamePath {
    NamePath::unresolved(path.iter().map(lower_name).collect())
}
