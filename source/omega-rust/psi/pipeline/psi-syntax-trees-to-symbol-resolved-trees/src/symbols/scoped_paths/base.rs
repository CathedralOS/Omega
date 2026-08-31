use psi_symbols::{SymbolHandle, SymbolKind, SymbolTable};

use super::super::lookup::{
    child_indexed_symbol_by_kinds, child_or_attached_data_child_symbol_by_kinds,
    child_symbol_by_kinds, top_level_symbol_by_kinds,
};

pub(super) fn resolve_base_symbol(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    member: &psi_symbol_resolved_trees::name::DiagnosticName,
) -> SymbolHandle {
    if state_symbol.is_valid() {
        let lexical_symbol = child_symbol_by_kinds(
            symbols,
            state_symbol,
            &[SymbolKind::Parameter, SymbolKind::Local],
            member.as_str(),
        );
        if lexical_symbol.is_valid() {
            return lexical_symbol;
        }
    }

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
        return machine_child;
    }

    top_level_symbol_by_kinds(
        symbols,
        &[
            SymbolKind::BuiltinType,
            SymbolKind::Data,
            SymbolKind::Machine,
            SymbolKind::Trait,
        ],
        member.as_str(),
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
