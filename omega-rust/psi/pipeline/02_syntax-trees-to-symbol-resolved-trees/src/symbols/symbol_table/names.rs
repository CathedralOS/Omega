use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{SymbolKind, SymbolNameRef};

pub(super) type SymbolSeed<'name> = (SymbolKind, SymbolNameRef<'name>);

pub(super) fn symbol_seed<'name>(
    kind: SymbolKind,
    name: &'name symbol_resolved_trees::name::DiagnosticName,
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

/// A machine declaration's seed. Compiler-synthesized machines (trait
/// defaults, equatable realizations) carry a generated name, so the authored
/// carrier occurrence in the conformance supplies the declaration's exact
/// provenance: that occurrence pins the conforming package's identity even
/// when the carrier declaration itself lives in another package.
pub(super) fn machine_symbol_seed<'name>(
    machine: &'name symbol_resolved_trees::machine::Machine,
    has_sources: bool,
) -> SymbolSeed<'name> {
    if has_sources
        && !machine.name.is_source_backed()
        && let Some(attached) = machine
            .attached_data
            .as_ref()
            .filter(|attached| attached.is_source_backed())
    {
        return (
            SymbolKind::Machine,
            SymbolNameRef::OwnedSource {
                value: machine.name.as_str(),
                source_span: attached.source_span(),
            },
        );
    }
    symbol_seed(SymbolKind::Machine, &machine.name, has_sources)
}

pub(super) fn operator_symbol_name(
    program: &SymbolResolvedTrees,
    operator: &symbol_resolved_trees::operator::OperatorDefinition,
) -> String {
    program
        .operator_path_members(operator.name)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::")
}

pub(super) fn measure_symbol_name(
    program: &SymbolResolvedTrees,
    measure: &symbol_resolved_trees::measure::MeasureDefinition,
) -> String {
    program
        .measure_path_members(measure.name)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::")
}

pub(super) fn measure_symbol_seed<'name>(
    program: &SymbolResolvedTrees,
    measure: &symbol_resolved_trees::measure::MeasureDefinition,
    canonical_name: &'name str,
    has_sources: bool,
) -> SymbolSeed<'name> {
    match program.measure_path_members(measure.name).last() {
        Some(name) if has_sources && name.is_source_backed() => (
            SymbolKind::Measure,
            SymbolNameRef::OwnedSource {
                value: canonical_name,
                source_span: name.source_span(),
            },
        ),
        _ => (SymbolKind::Measure, SymbolNameRef::Borrowed(canonical_name)),
    }
}

pub(super) fn operator_symbol_seed<'name>(
    program: &SymbolResolvedTrees,
    operator: &symbol_resolved_trees::operator::OperatorDefinition,
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
