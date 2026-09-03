use psi_symbols::{SymbolHandle, SymbolKind, SymbolTable};

mod base;

use self::base::{resolve_base_indexed_symbol, resolve_base_symbol};
use super::lookup::{child_indexed_symbol_by_kinds, child_or_attached_data_child_symbol_by_kinds};

pub(super) fn resolve_state_scoped_table_path(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &psi_symbol_resolved_trees::expression::ExpressionTable,
    path: &psi_symbol_resolved_trees::expression::TableNamePath,
) -> (SymbolHandle, SymbolHandle) {
    let members = expression_table.name_path_members(path.members);
    resolve_state_scoped_table_members(
        symbols,
        machine_symbol,
        state_symbol,
        members,
        path.is_self_value,
        None,
    )
}

/// Resolve every authored segment of a table name path. An empty result means
/// that at least one segment could not be resolved; callers must not retain a
/// partially valid path because that would hide the exact failed selection.
pub(super) fn resolve_state_scoped_table_path_member_symbols(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &psi_symbol_resolved_trees::expression::ExpressionTable,
    path: &psi_symbol_resolved_trees::expression::TableNamePath,
) -> Vec<SymbolHandle> {
    let members = expression_table.name_path_members(path.members);
    if members.is_empty() {
        return Vec::new();
    }

    let mut resolved = Vec::with_capacity(members.len());
    let mut index = 0usize;
    let mut current = SymbolHandle::invalid();

    if path.is_self_value {
        current = machine_symbol;
        resolved.push(current);
        index = 1;
    }

    if index < members.len() && !current.is_valid() {
        current = resolve_base_symbol(symbols, machine_symbol, state_symbol, &members[index]);
        if !current.is_valid() {
            return Vec::new();
        }
        resolved.push(current);
        index += 1;
    } else if index < members.len() {
        current = child_or_attached_data_child_symbol_by_kinds(
            symbols,
            current,
            &[SymbolKind::Field, SymbolKind::State],
            members[index].as_str(),
        );
        if !current.is_valid() {
            return Vec::new();
        }
        resolved.push(current);
        index += 1;
    }

    for member in &members[index..] {
        current = child_or_attached_data_child_symbol_by_kinds(
            symbols,
            current,
            &[
                SymbolKind::Field,
                SymbolKind::State,
                SymbolKind::Parameter,
                SymbolKind::Variant,
            ],
            member.as_str(),
        );
        if !current.is_valid() {
            return Vec::new();
        }
        resolved.push(current);
    }

    if resolved.len() == members.len() {
        resolved
    } else {
        Default::default()
    }
}

pub(super) fn resolve_state_scoped_table_path_with_indexed_last_member(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &psi_symbol_resolved_trees::expression::ExpressionTable,
    path: &psi_symbol_resolved_trees::expression::TableNamePath,
    index: i64,
) -> (SymbolHandle, SymbolHandle) {
    let members = expression_table.name_path_members(path.members);
    resolve_state_scoped_table_members(
        symbols,
        machine_symbol,
        state_symbol,
        members,
        path.is_self_value,
        Some(index),
    )
}

pub(super) fn resolve_state_scoped_members(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    members: &[psi_symbol_resolved_trees::name::DiagnosticName],
    starts_at_self: bool,
) -> (SymbolHandle, SymbolHandle) {
    if members.is_empty() {
        return invalid_symbol_pair();
    }

    let mut index = 0usize;
    let mut current = SymbolHandle::invalid();
    let head: SymbolHandle;

    if starts_at_self {
        current = machine_symbol;
        index = 1;
    }

    if index >= members.len() {
        return if current.is_valid() {
            (current, current)
        } else {
            invalid_symbol_pair()
        };
    }

    if !current.is_valid() {
        current = resolve_base_symbol(symbols, machine_symbol, state_symbol, &members[index]);
        if !current.is_valid() {
            return invalid_symbol_pair();
        }
        head = current;
        index += 1;
    } else {
        current = child_or_attached_data_child_symbol_by_kinds(
            symbols,
            current,
            &[SymbolKind::Field, SymbolKind::State],
            members[index].as_str(),
        );
        if !current.is_valid() {
            return invalid_symbol_pair();
        }
        head = current;
        index += 1;
    }

    for member in &members[index..] {
        current = child_or_attached_data_child_symbol_by_kinds(
            symbols,
            current,
            &[
                SymbolKind::Field,
                SymbolKind::State,
                SymbolKind::Parameter,
                SymbolKind::Variant,
            ],
            member.as_str(),
        );
        if !current.is_valid() {
            return invalid_symbol_pair();
        }
    }

    (head, current)
}

fn resolve_state_scoped_table_members(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    members: &[psi_symbol_resolved_trees::name::DiagnosticName],
    starts_at_self: bool,
    indexed_last_member: Option<i64>,
) -> (SymbolHandle, SymbolHandle) {
    if members.is_empty() {
        return invalid_symbol_pair();
    }

    let mut index = 0usize;
    let mut current = SymbolHandle::invalid();
    let head: SymbolHandle;

    if starts_at_self {
        current = machine_symbol;
        index = 1;
    }

    if index >= members.len() {
        return if current.is_valid() {
            (current, current)
        } else {
            invalid_symbol_pair()
        };
    }

    if !current.is_valid() {
        current =
            if let Some(last_index) = indexed_last_member.filter(|_| index + 1 == members.len()) {
                let indexed_symbol = resolve_base_indexed_symbol(
                    symbols,
                    machine_symbol,
                    state_symbol,
                    members[index].as_str(),
                    last_index,
                );
                if !indexed_symbol.is_valid() {
                    return invalid_symbol_pair();
                }
                indexed_symbol
            } else {
                let base_symbol =
                    resolve_base_symbol(symbols, machine_symbol, state_symbol, &members[index]);
                if !base_symbol.is_valid() {
                    return invalid_symbol_pair();
                }
                base_symbol
            };
        head = current;
        index += 1;
    } else {
        current =
            if let Some(last_index) = indexed_last_member.filter(|_| index + 1 == members.len()) {
                child_indexed_symbol_by_kinds(
                    symbols,
                    current,
                    &[SymbolKind::Field, SymbolKind::State],
                    members[index].as_str(),
                    last_index,
                )
            } else {
                child_or_attached_data_child_symbol_by_kinds(
                    symbols,
                    current,
                    &[SymbolKind::Field, SymbolKind::State],
                    members[index].as_str(),
                )
            };
        if !current.is_valid() {
            return invalid_symbol_pair();
        }
        head = current;
        index += 1;
    }

    for (offset, member) in members[index..].iter().enumerate() {
        let is_last = index + offset + 1 == members.len();
        current = if let Some(last_index) = indexed_last_member.filter(|_| is_last) {
            child_indexed_symbol_by_kinds(
                symbols,
                current,
                &[
                    SymbolKind::Field,
                    SymbolKind::State,
                    SymbolKind::Parameter,
                    SymbolKind::Variant,
                ],
                member.as_str(),
                last_index,
            )
        } else {
            child_or_attached_data_child_symbol_by_kinds(
                symbols,
                current,
                &[
                    SymbolKind::Field,
                    SymbolKind::State,
                    SymbolKind::Parameter,
                    SymbolKind::Variant,
                ],
                member.as_str(),
            )
        };
        if !current.is_valid() {
            return invalid_symbol_pair();
        }
    }

    (head, current)
}

pub(super) fn invalid_symbol_pair() -> (SymbolHandle, SymbolHandle) {
    (SymbolHandle::invalid(), SymbolHandle::invalid())
}
