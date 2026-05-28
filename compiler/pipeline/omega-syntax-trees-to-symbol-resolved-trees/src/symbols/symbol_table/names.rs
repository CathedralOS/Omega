use omega_core::symbols::{SymbolKind, SymbolNameRef};
use omega_symbol_resolved_trees::SymbolResolvedTrees;

pub(super) type SymbolSeed<'name> = (SymbolKind, SymbolNameRef<'name>);

pub(super) fn symbol_seed<'name>(
    kind: SymbolKind,
    name: &'name omega_symbol_resolved_trees::name::DiagnosticName,
    has_sources: bool,
) -> SymbolSeed<'name> {
    if has_sources && name.is_source_backed() {
        (kind, SymbolNameRef::Source(name.source_span()))
    } else {
        (kind, SymbolNameRef::Borrowed(name.as_str()))
    }
}

pub(super) fn operator_symbol_name(
    program: &SymbolResolvedTrees,
    operator: &omega_symbol_resolved_trees::operator::OperatorDefinition,
) -> String {
    program
        .operator_path_members(operator.name)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::")
}
