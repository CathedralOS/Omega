use symbols::{SymbolHandle, SymbolKind, SymbolTable};

mod base;

use self::base::{resolve_base_indexed_symbol, resolve_base_symbol};
use super::lookup::{child_indexed_symbol_by_kinds, child_or_attached_data_child_symbol_by_kinds};

pub(super) fn resolve_state_scoped_table_path(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &symbol_resolved_trees::expression::ExpressionTable,
    path: &symbol_resolved_trees::expression::TableNamePath,
) -> (SymbolHandle, SymbolHandle) {
    let members = expression_table.name_path_members(path.members);
    resolve_state_scoped_table_members(symbols, machine_symbol, state_symbol, members, None)
}

/// Resolve every authored segment of a table name path. An empty result means
/// that at least one segment could not be resolved; callers must not retain a
/// partially valid path because that would hide the exact failed selection.
pub(super) fn resolve_state_scoped_table_path_member_symbols(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &symbol_resolved_trees::expression::ExpressionTable,
    path: &symbol_resolved_trees::expression::TableNamePath,
) -> Vec<SymbolHandle> {
    let members = expression_table.name_path_members(path.members);
    if members.is_empty() {
        return Vec::new();
    }

    let head = resolve_base_symbol(symbols, machine_symbol, state_symbol, &members[0]);
    if !head.is_valid() {
        return Vec::new();
    }
    let resolved = resolve_path_members_from_head(symbols, head, &members[1..]);

    if resolved.len() == members.len() {
        resolved
    } else {
        Default::default()
    }
}

/// A failed suffix does not invalidate the caller's independently selected
/// lexical head, but it cannot publish a complete member-selection chain.
pub(super) fn resolve_path_members_from_head(
    symbols: &SymbolTable,
    head: SymbolHandle,
    suffix: &[symbol_resolved_trees::name::DiagnosticName],
) -> Vec<SymbolHandle> {
    let mut current = head;
    let mut resolved = vec![head];
    for member in suffix {
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

    resolved
}

pub(super) fn resolve_state_scoped_table_path_with_indexed_last_member(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &symbol_resolved_trees::expression::ExpressionTable,
    path: &symbol_resolved_trees::expression::TableNamePath,
    index: i64,
) -> (SymbolHandle, SymbolHandle) {
    let members = expression_table.name_path_members(path.members);
    resolve_state_scoped_table_members(symbols, machine_symbol, state_symbol, members, Some(index))
}

pub(super) fn resolve_state_scoped_members(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    members: &[symbol_resolved_trees::name::DiagnosticName],
) -> (SymbolHandle, SymbolHandle) {
    if members.is_empty() {
        return invalid_symbol_pair();
    }

    let head = resolve_base_symbol(symbols, machine_symbol, state_symbol, &members[0]);
    if !head.is_valid() {
        return invalid_symbol_pair();
    }
    let mut current = head;

    for member in &members[1..] {
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
            return (head, SymbolHandle::invalid());
        }
    }

    (head, current)
}

fn resolve_state_scoped_table_members(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    members: &[symbol_resolved_trees::name::DiagnosticName],
    indexed_last_member: Option<i64>,
) -> (SymbolHandle, SymbolHandle) {
    if members.is_empty() {
        return invalid_symbol_pair();
    }

    let head = if let Some(last_index) = indexed_last_member.filter(|_| members.len() == 1) {
        resolve_base_indexed_symbol(
            symbols,
            machine_symbol,
            state_symbol,
            members[0].as_str(),
            last_index,
        )
    } else {
        resolve_base_symbol(symbols, machine_symbol, state_symbol, &members[0])
    };
    if !head.is_valid() {
        return invalid_symbol_pair();
    }
    let mut current = head;

    for (offset, member) in members[1..].iter().enumerate() {
        let is_last = offset + 2 == members.len();
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
            return (head, SymbolHandle::invalid());
        }
    }

    (head, current)
}

pub(super) fn invalid_symbol_pair() -> (SymbolHandle, SymbolHandle) {
    (SymbolHandle::invalid(), SymbolHandle::invalid())
}
