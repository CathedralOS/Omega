use symbols::{SymbolHandle, SymbolKind, SymbolTable};

use super::super::lookup::{
    child_indexed_symbol_by_kinds, child_or_attached_data_child_symbol_by_kinds,
};
use super::super::scoped_paths::{invalid_symbol_pair, resolve_state_scoped_table_path};
use symbol_resolved_trees::expression::ExpressionNode;

/// The member kinds a receiver path may resolve through — fields, states,
/// parameters and variants.
const RECEIVER_MEMBER_KINDS: &[SymbolKind] = &[
    SymbolKind::Field,
    SymbolKind::State,
    SymbolKind::Parameter,
    SymbolKind::Variant,
];

/// A data head selects common fields; a case head selects only its payload.
/// Sibling payload namespaces never participate, and duplicate spellings do
/// not become declaration-order selection.
pub(in crate::symbols) fn pattern_field_symbol(
    symbols: &SymbolTable,
    head: SymbolHandle,
    spelling: &str,
) -> SymbolHandle {
    if !head.is_valid() {
        return SymbolHandle::invalid();
    }
    let mut fields = symbols
        .child_handles(head)
        .into_iter()
        .flatten()
        .filter(|field| {
            symbols.get(*field).kind == SymbolKind::Field && symbols.name(*field) == spelling
        });
    let Some(field) = fields.next() else {
        return SymbolHandle::invalid();
    };
    if fields.next().is_some() {
        SymbolHandle::invalid()
    } else {
        field
    }
}

/// Retain the field identities selected by a resolved pattern head. Contracts
/// and executable matches carry the same selectors into type checking.
pub(in crate::symbols) fn assign_pattern_field_symbols(
    symbols: &SymbolTable,
    expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
    pattern: symbol_resolved_trees::expression::TableDestructurePattern,
) {
    let head = expression_table
        .name_path_node(pattern.head)
        .map(|path| path.symbol)
        .unwrap_or_default();
    for field_handle in pattern.fields.handles() {
        let field = expression_table.match_fields(pattern.fields)
            [(field_handle.arena_index() - pattern.fields.start().arena_index()) as usize]
            .clone();
        expression_table.match_field_mut(field_handle).field_symbol =
            pattern_field_symbol(symbols, head, field.member.as_str());
    }
}

pub(in crate::symbols) fn resolve_expression_table_member_symbol(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &symbol_resolved_trees::expression::ExpressionTable,
    receiver: symbol_resolved_trees::expression::ExpressionHandle,
    member: &symbol_resolved_trees::name::DiagnosticName,
) -> SymbolHandle {
    if let ExpressionNode::SavedSubject(saved) = expression_table.expression(receiver) {
        if saved.component == language_core::PatternComponent::Case {
            return SymbolHandle::invalid();
        }
        let head = expression_table
            .name_path_node(saved.pattern_head)
            .map(|path| path.symbol)
            .unwrap_or_default();
        return pattern_field_symbol(symbols, head, member.as_str());
    }
    let (_, receiver_symbol) = receiver_path_symbols(
        symbols,
        machine_symbol,
        state_symbol,
        expression_table,
        receiver,
        None,
    );
    if !receiver_symbol.is_valid() {
        return SymbolHandle::invalid();
    }

    child_or_attached_data_child_symbol_by_kinds(
        symbols,
        receiver_symbol,
        RECEIVER_MEMBER_KINDS,
        member.as_str(),
    )
}

/// Resolve a receiver expression to its lexical head and selected leaf.
pub(in crate::symbols) fn resolve_expression_table_receiver_path_symbols(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &symbol_resolved_trees::expression::ExpressionTable,
    receiver: symbol_resolved_trees::expression::ExpressionHandle,
) -> (SymbolHandle, SymbolHandle) {
    receiver_path_symbols(
        symbols,
        machine_symbol,
        state_symbol,
        expression_table,
        receiver,
        None,
    )
}

/// One walk over a receiver expression. `indexed_last_member` carries the
/// receiver's possible indexed tail: the outermost `Indexed` supplies it
/// (`items[index]`), and only the final member's lookup consults the indexed
/// declaration form. Names, members and borrows resolve the same way
/// otherwise, and a nested index inside another index has no declaration form.
/// A matched subject resolves through its pattern head, a declaration path
/// rather than an indexed element, and its selected member through the
/// structural head's own field roster.
fn receiver_path_symbols(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &symbol_resolved_trees::expression::ExpressionTable,
    receiver: symbol_resolved_trees::expression::ExpressionHandle,
    indexed_last_member: Option<i64>,
) -> (SymbolHandle, SymbolHandle) {
    use symbol_resolved_trees::expression::ExpressionNode;
    match expression_table.expression(receiver) {
        ExpressionNode::SavedSubject(saved) if indexed_last_member.is_none() => {
            receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                saved.pattern_head,
                None,
            )
        }
        ExpressionNode::Name(path) => resolve_state_scoped_table_path(
            symbols,
            machine_symbol,
            state_symbol,
            expression_table,
            path,
            indexed_last_member,
        ),
        ExpressionNode::Member(member) => {
            let (head_symbol, receiver_symbol) = receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                member.receiver,
                None,
            );
            if !receiver_symbol.is_valid() {
                return invalid_symbol_pair();
            }
            let member_symbol = match indexed_last_member {
                Some(index) => child_indexed_symbol_by_kinds(
                    symbols,
                    receiver_symbol,
                    RECEIVER_MEMBER_KINDS,
                    member.member.as_str(),
                    index,
                ),
                None if matches!(
                    expression_table.expression(member.receiver),
                    ExpressionNode::SavedSubject(_)
                ) =>
                {
                    pattern_field_symbol(symbols, receiver_symbol, member.member.as_str())
                }
                None => child_or_attached_data_child_symbol_by_kinds(
                    symbols,
                    receiver_symbol,
                    RECEIVER_MEMBER_KINDS,
                    member.member.as_str(),
                ),
            };

            if member_symbol.is_valid() {
                (head_symbol, member_symbol)
            } else {
                invalid_symbol_pair()
            }
        }
        ExpressionNode::Borrow(inner) => receiver_path_symbols(
            symbols,
            machine_symbol,
            state_symbol,
            expression_table,
            inner.target,
            indexed_last_member,
        ),
        ExpressionNode::Indexed(indexed) if indexed_last_member.is_none() => {
            let ExpressionNode::Integer(index) = expression_table.expression(indexed.index) else {
                return invalid_symbol_pair();
            };
            // An index beyond i64 cannot name a real element.
            let Some(index) = index.value_i64() else {
                return invalid_symbol_pair();
            };

            receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                indexed.collection,
                Some(index),
            )
        }
        _ => invalid_symbol_pair(),
    }
}
