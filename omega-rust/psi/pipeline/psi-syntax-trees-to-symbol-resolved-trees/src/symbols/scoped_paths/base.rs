use psi_symbols::{SymbolHandle, SymbolKind, SymbolTable};

use super::super::lookup::{
    child_indexed_symbol_by_kinds, child_or_attached_data_child_symbol_by_kinds,
    child_symbol_by_kinds,
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

    let entry_symbol = enclosing_entry_state_symbol(symbols, machine_symbol, state_symbol);
    if entry_symbol.is_valid() {
        let captured_symbol = child_symbol_by_kinds(
            symbols,
            entry_symbol,
            &[SymbolKind::Parameter, SymbolKind::Local],
            member.as_str(),
        );
        if captured_symbol.is_valid() {
            return captured_symbol;
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

    symbols
        .find_top_level_by_name_and_kinds_from_source(
            member.as_str(),
            &[
                SymbolKind::BuiltinType,
                SymbolKind::Data,
                SymbolKind::Machine,
                SymbolKind::Trait,
            ],
            member.source_span(),
        )
        .unwrap_or_else(SymbolHandle::invalid)
}

/// A non-entry state is authored lexically inside the machine's entry block,
/// so the entry telescope and the entry block's own `let` bindings are in
/// scope there. `parse_state` admits no nested `state` member, so states are
/// flat children of the machine symbol and the enclosing lexical scope of
/// every non-entry state is the machine's first state.
///
/// The captured binding is searched after the state's own telescope and before
/// the machine's fields, which is the shadowing order the entry state itself
/// already has.
fn enclosing_entry_state_symbol(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> SymbolHandle {
    if !state_symbol.is_valid() {
        return SymbolHandle::invalid();
    }
    let Some(children) = symbols.child_handles(machine_symbol) else {
        return SymbolHandle::invalid();
    };
    let entry_symbol = children
        .into_iter()
        .find(|child| symbols.get(*child).kind == SymbolKind::State)
        .unwrap_or_else(SymbolHandle::invalid);
    if entry_symbol == state_symbol {
        return SymbolHandle::invalid();
    }
    entry_symbol
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
