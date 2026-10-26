use symbols::{SymbolHandle, SymbolKind, SymbolLookup, SymbolTable};

use super::super::lookup::{
    child_indexed_symbol_by_kinds, child_or_attached_data_child_symbol_by_kinds,
    child_symbol_by_kinds,
};

/// Preserve absence versus ambiguity for compound namespace selection. Only
/// absence permits a longer certified address; ambiguity cannot be resolved by
/// choosing the sole declaration that happens to have the requested descendant.
pub(super) fn resolve_base_lookup(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    member: &symbol_resolved_trees::name::DiagnosticName,
) -> SymbolLookup {
    if state_symbol.is_valid() {
        let lexical_symbol = child_symbol_by_kinds(
            symbols,
            state_symbol,
            &[SymbolKind::Parameter, SymbolKind::Local],
            member.as_str(),
        );
        if lexical_symbol.is_valid() {
            return SymbolLookup::Unique(lexical_symbol);
        }
    }

    // Named states have their own value frontier. Entry parameters and locals
    // reach another state only through its explicit transition arguments.
    let machine_child = child_or_attached_data_child_symbol_by_kinds(
        symbols,
        machine_symbol,
        &[
            SymbolKind::Field,
            SymbolKind::State,
            SymbolKind::ConformanceParameter,
        ],
        member.as_str(),
    );
    if machine_child.is_valid() {
        return SymbolLookup::Unique(machine_child);
    }

    let lexical =
        super::super::lookup::lexical_machine_lookup(symbols, machine_symbol, member.as_str());
    if !matches!(lexical, SymbolLookup::NotFound) {
        return lexical;
    }

    symbols.lookup_top_level_by_name_and_kinds_from_source_matching(
        member.as_str(),
        &[
            SymbolKind::BuiltinType,
            SymbolKind::Const,
            SymbolKind::Data,
            SymbolKind::Machine,
            SymbolKind::Trait,
            SymbolKind::Conformance,
            SymbolKind::Module,
        ],
        member.source_span(),
        |_| true,
    )
}

pub(super) fn resolve_base_indexed_symbol(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    member: &str,
    index: i64,
) -> SymbolHandle {
    if state_symbol.is_valid() {
        let parameter_symbol = child_indexed_symbol_by_kinds(
            symbols,
            state_symbol,
            &[SymbolKind::Parameter],
            member,
            index,
        );
        if parameter_symbol.is_valid() {
            return parameter_symbol;
        }
    }

    child_indexed_symbol_by_kinds(
        symbols,
        machine_symbol,
        &[SymbolKind::Field, SymbolKind::State],
        member,
        index,
    )
}
