use symbols::{JoinedPath, SymbolHandle, SymbolKind, SymbolLookup, SymbolTable};

mod base;

use self::base::resolve_base_indexed_symbol;
use super::lookup::{child_indexed_symbol_by_kinds, child_or_attached_data_child_symbol_by_kinds};

/// The symbol kinds a scoped path may resolve through at each member:
/// fields, states, parameters, variants and the mathematical/constant
/// declarations — the same roster every path-walk step consults.
const SCOPED_PATH_MEMBER_KINDS: &[SymbolKind] = &[
    SymbolKind::Field,
    SymbolKind::State,
    SymbolKind::Parameter,
    SymbolKind::Variant,
    SymbolKind::MathematicalDefinition,
    SymbolKind::Const,
];

/// Resolve a state-scoped table path to its lexical head and selected leaf.
/// `indexed_last_member` carries the path's possible indexed tail
/// (`items[index]`): one walk serves both, and only the final member's lookup
/// consults the indexed declaration form.
pub(super) fn resolve_state_scoped_table_path(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &symbol_resolved_trees::expression::ExpressionTable,
    path: &symbol_resolved_trees::expression::TableNamePath,
    indexed_last_member: Option<i64>,
) -> (SymbolHandle, SymbolHandle) {
    let members = expression_table.name_path_members(path.members);
    resolve_state_scoped_members(
        symbols,
        machine_symbol,
        state_symbol,
        members,
        indexed_last_member,
    )
}

/// Retain each authored segment's declaration where it has one. Package-alias
/// segments have zero identities; their complete prefix selects one exact
/// declaration. An empty result means that selection or a descendant failed,
/// so callers must not retain a partially selected suffix.
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

    let (head, prefix_length) = resolve_path_head(symbols, machine_symbol, state_symbol, members);
    if !head.is_valid() {
        return Vec::new();
    }
    let suffix = resolve_path_members_from_head(symbols, head, &members[prefix_length..]);
    // Package aliases are address segments, not invented declaration symbols.
    // The selected declaration owns the static prefix; only real descendants
    // contribute identities to the remaining member-selection chain.
    let mut resolved = vec![SymbolHandle::invalid(); prefix_length - 1];
    resolved.extend(suffix);

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
            SCOPED_PATH_MEMBER_KINDS,
            member.as_str(),
        );
        if !current.is_valid() {
            return Vec::new();
        }
        resolved.push(current);
    }

    resolved
}

/// Resolve a run of authored path members to the lexical head and the selected
/// leaf. `indexed_last_member` is the walk's only variation: when present, the
/// final member's declaration is looked up in its indexed form (`name[index]`)
/// instead of its plain form, and only at that final member.
pub(super) fn resolve_state_scoped_members(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    members: &[symbol_resolved_trees::name::DiagnosticName],
    indexed_last_member: Option<i64>,
) -> (SymbolHandle, SymbolHandle) {
    if members.is_empty() {
        return invalid_symbol_pair();
    }

    let (head, prefix_length) =
        if let Some(last_index) = indexed_last_member.filter(|_| members.len() == 1) {
            (
                resolve_base_indexed_symbol(
                    symbols,
                    machine_symbol,
                    state_symbol,
                    members[0].as_str(),
                    last_index,
                ),
                1,
            )
        } else {
            resolve_path_head(symbols, machine_symbol, state_symbol, members)
        };
    if !head.is_valid() {
        return invalid_symbol_pair();
    }
    let mut current = head;

    for (offset, member) in members[prefix_length..].iter().enumerate() {
        let is_last = offset + prefix_length + 1 == members.len();
        current = if let Some(last_index) = indexed_last_member.filter(|_| is_last) {
            child_indexed_symbol_by_kinds(
                symbols,
                current,
                SCOPED_PATH_MEMBER_KINDS,
                member.as_str(),
                last_index,
            )
        } else {
            child_or_attached_data_child_symbol_by_kinds(
                symbols,
                current,
                SCOPED_PATH_MEMBER_KINDS,
                member.as_str(),
            )
        };
        if !current.is_valid() {
            return (head, SymbolHandle::invalid());
        }
    }

    (head, current)
}

/// Lexical values own their suffix before static-address lookup. When the
/// first segment is not a declaration (a loader-certified package alias),
/// select the longest exact declaration prefix through ordinary name law.
/// This is the same operation for a call receiver and an ordinary name path;
/// aliases never become runtime values or grant visibility to another package.
fn resolve_path_head(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    members: &[symbol_resolved_trees::name::DiagnosticName],
) -> (SymbolHandle, usize) {
    match base::resolve_base_lookup(symbols, machine_symbol, state_symbol, &members[0]) {
        SymbolLookup::Unique(head) => return (head, 1),
        SymbolLookup::Ambiguous { .. } => return (SymbolHandle::invalid(), 1),
        SymbolLookup::NotFound => {}
    }
    for prefix_length in (2..=members.len()).rev() {
        let prefix = &members[..prefix_length];
        let name = symbol_resolved_trees::name::DiagnosticName::new(
            prefix.joined_path(),
            super::lookup::diagnostic_path_source_span(prefix),
        );
        match base::resolve_base_lookup(symbols, machine_symbol, state_symbol, &name) {
            SymbolLookup::Unique(selected) => return (selected, prefix_length),
            SymbolLookup::Ambiguous { .. } => return (SymbolHandle::invalid(), 1),
            SymbolLookup::NotFound => {}
        }
    }
    (SymbolHandle::invalid(), 1)
}

pub(super) fn invalid_symbol_pair() -> (SymbolHandle, SymbolHandle) {
    (SymbolHandle::invalid(), SymbolHandle::invalid())
}
