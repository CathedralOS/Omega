use psi_symbols::{SymbolHandle, SymbolKind, SymbolTable};

pub(super) fn diagnostic_path_source_span(
    members: &[psi_symbol_resolved_trees::name::DiagnosticName],
) -> psi_source::SourceSpan {
    let Some(first) = members.first() else {
        return psi_source::SourceSpan::default();
    };
    let Some(last) = members.last() else {
        return first.source_span();
    };
    if first.source_span().source_id != last.source_span().source_id {
        return first.source_span();
    }
    psi_source::SourceSpan::new(
        first.source_span().source_id,
        psi_source::Span::new(first.source_span().span.start, last.source_span().span.end),
    )
}

pub(super) fn top_level_type_symbol_for_source(
    symbols: &SymbolTable,
    name: &psi_symbol_resolved_trees::name::DiagnosticName,
) -> SymbolHandle {
    symbols
        .find_top_level_by_name_and_kinds_from_source(
            name.as_str(),
            &[
                SymbolKind::BuiltinType,
                SymbolKind::Data,
                SymbolKind::Machine,
                SymbolKind::Trait,
            ],
            name.source_span(),
        )
        .unwrap_or_else(SymbolHandle::invalid)
}

pub(super) fn top_level_symbol_for_source(
    symbols: &SymbolTable,
    kind: SymbolKind,
    name: &psi_symbol_resolved_trees::name::DiagnosticName,
) -> SymbolHandle {
    symbols
        .find_top_level_by_name_and_kinds_from_source(name.as_str(), &[kind], name.source_span())
        .unwrap_or_else(SymbolHandle::invalid)
}

pub(super) fn top_level_symbol_by_kinds(
    symbols: &SymbolTable,
    kinds: &[SymbolKind],
    name: &str,
) -> SymbolHandle {
    child_symbol_by_kinds(symbols, symbols.root(), kinds, name)
}

pub(super) fn child_symbol_by_kinds(
    symbols: &SymbolTable,
    parent: SymbolHandle,
    kinds: &[SymbolKind],
    name: &str,
) -> SymbolHandle {
    child_symbol_by_kinds_matching(symbols, parent, kinds, |symbol_name| symbol_name == name)
}

pub(super) fn child_or_attached_data_child_symbol_by_kinds(
    symbols: &SymbolTable,
    parent: SymbolHandle,
    kinds: &[SymbolKind],
    name: &str,
) -> SymbolHandle {
    let child = child_symbol_by_kinds(symbols, parent, kinds, name);
    if child.is_valid() || symbols.get(parent).kind != SymbolKind::Machine {
        return child;
    }

    let Some(attached_data_name) = symbols.name(parent).split_once("::").map(|(data, _)| data)
    else {
        return SymbolHandle::invalid();
    };
    let reference = symbols
        .symbol_provenance_source_span(parent)
        .unwrap_or_default();
    let attached_data = symbols
        .find_top_level_by_name_and_kinds_from_source(
            attached_data_name,
            &[SymbolKind::Data],
            reference,
        )
        .unwrap_or_else(SymbolHandle::invalid);
    if !attached_data.is_valid() {
        return SymbolHandle::invalid();
    }

    child_symbol_by_kinds(symbols, attached_data, kinds, name)
}

pub(super) fn child_indexed_symbol_by_kinds(
    symbols: &SymbolTable,
    parent: SymbolHandle,
    kinds: &[SymbolKind],
    name: &str,
    index: i64,
) -> SymbolHandle {
    child_symbol_by_kinds_matching(symbols, parent, kinds, |symbol_name| {
        symbol_name_matches_indexed_member(symbol_name, name, index)
    })
}

pub(super) fn call_target_for_attached_data(
    symbols: &SymbolTable,
    attached_data: &str,
    target_name: &str,
    reference: psi_source::SourceSpan,
) -> SymbolHandle {
    let machine_name = format!("{attached_data}::{target_name}");
    let machine_symbol = symbols
        .find_top_level_by_name_and_kinds_from_source(
            &machine_name,
            &[SymbolKind::Machine],
            reference,
        )
        .unwrap_or_else(SymbolHandle::invalid);
    if !machine_symbol.is_valid() {
        return SymbolHandle::invalid();
    }

    child_symbol_by_kinds(symbols, machine_symbol, &[SymbolKind::State], target_name)
}

fn child_symbol_by_kinds_matching(
    symbols: &SymbolTable,
    parent: SymbolHandle,
    kinds: &[SymbolKind],
    mut matches_name: impl FnMut(&str) -> bool,
) -> SymbolHandle {
    let Some(children) = symbols.child_handles(parent) else {
        return SymbolHandle::invalid();
    };

    for child in children {
        let symbol = symbols.get(child);
        if matches_name(symbols.name(child)) && kinds.contains(&symbol.kind) {
            return child;
        }
    }

    SymbolHandle::invalid()
}

fn symbol_name_matches_indexed_member(symbol_name: &str, member: &str, index: i64) -> bool {
    let Some(suffix) = symbol_name.strip_prefix(member) else {
        return false;
    };
    let Some(suffix) = suffix.strip_prefix('[') else {
        return false;
    };
    let Some(index_text) = suffix.strip_suffix(']') else {
        return false;
    };

    index_text.parse::<i64>().ok() == Some(index)
}
