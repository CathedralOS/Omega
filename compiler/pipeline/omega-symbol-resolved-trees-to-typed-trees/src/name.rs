use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(crate) fn lower_name(name: &resolved::name::DiagnosticName) -> typed::name::ProgramName {
    typed::name::ProgramName::generated(name.as_str())
}

pub(crate) fn lower_statement_name_path(
    members: &[resolved::name::DiagnosticName],
    head_symbol: omega_core::symbols::SymbolHandle,
    symbol: omega_core::symbols::SymbolHandle,
) -> typed::expression::NamePath {
    typed::expression::NamePath::resolved(
        members.iter().map(lower_name).collect(),
        head_symbol,
        symbol,
    )
}
