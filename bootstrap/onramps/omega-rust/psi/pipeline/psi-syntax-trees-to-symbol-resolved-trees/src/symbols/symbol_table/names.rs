use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_symbols::{SymbolKind, SymbolNameRef};

pub(super) type SymbolSeed<'name> = (SymbolKind, SymbolNameRef<'name>);

pub(super) fn symbol_seed<'name>(
    kind: SymbolKind,
    name: &'name psi_symbol_resolved_trees::name::DiagnosticName,
    has_sources: bool,
) -> SymbolSeed<'name> {
    if has_sources && name.is_source_backed() {
        (
            kind,
            SymbolNameRef::OwnedSource {
                value: name.as_str(),
                source_span: name.source_span(),
            },
        )
    } else {
        (kind, SymbolNameRef::Borrowed(name.as_str()))
    }
}

pub(super) fn operator_symbol_name(
    program: &SymbolResolvedTrees,
    operator: &psi_symbol_resolved_trees::operator::OperatorDefinition,
) -> String {
    program
        .operator_path_members(operator.name)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::")
}

pub(super) fn operator_symbol_seed<'name>(
    program: &SymbolResolvedTrees,
    operator: &psi_symbol_resolved_trees::operator::OperatorDefinition,
    canonical_name: &'name str,
    has_sources: bool,
) -> SymbolSeed<'name> {
    let source_name = program.operator_path_members(operator.name).last();
    if has_sources && source_name.is_some_and(|name| name.is_source_backed()) {
        (
            SymbolKind::Operator,
            SymbolNameRef::OwnedSource {
                value: canonical_name,
                source_span: source_name
                    .expect("source-backed operator path member")
                    .source_span(),
            },
        )
    } else {
        (
            SymbolKind::Operator,
            SymbolNameRef::Borrowed(canonical_name),
        )
    }
}
