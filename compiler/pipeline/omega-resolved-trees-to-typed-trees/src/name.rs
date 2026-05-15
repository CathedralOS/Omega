use omega_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(crate) fn lower_name(name: &resolved::name::DiagnosticName) -> typed::name::ProgramName {
    typed::name::ProgramName::generated(name.as_str())
}

pub(crate) fn lower_name_path(
    path: &resolved::expression::NamePath,
) -> typed::expression::NamePath {
    let members = path.members().iter().map(lower_name).collect();
    typed::expression::NamePath::resolved(members, path.head_symbol(), path.symbol())
}
