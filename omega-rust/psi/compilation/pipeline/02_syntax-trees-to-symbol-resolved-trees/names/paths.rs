use crate::source_symbols::{JoinedPath, SymbolLookup, SymbolTable};
use symbols::{SymbolHandle, SymbolKind};

mod base {
    use crate::source_symbols::{SymbolLookup, SymbolTable};
    use symbols::{SymbolHandle, SymbolKind};

    use super::super::lookup::{
        child_indexed_symbol_by_kinds, child_or_attached_data_child_symbol_by_kinds,
        child_symbol_by_kinds,
    };

    /*
        Preserve absence versus ambiguity for compound namespace selection. Only
        absence permits a longer certified address; ambiguity cannot be resolved by
        choosing the sole declaration that happens to have the requested descendant.
    */
    pub(super) fn resolve_base_lookup(
        symbols: &SymbolTable,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        member: &crate::source_resolved::name::DiagnosticName,
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

        let lexical = super::super::lookup::lexical_declaration_lookup(
            symbols,
            machine_symbol,
            member.as_str(),
            &[
                SymbolKind::Machine,
                SymbolKind::TypeParameter,
                SymbolKind::MachineParameter,
                SymbolKind::ConformanceParameter,
                SymbolKind::PropositionParameter,
                SymbolKind::PropositionMachineParameter,
            ],
        );
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
}

use self::base::resolve_base_indexed_symbol;
use super::lookup::{child_indexed_symbol_by_kinds, child_or_attached_data_child_symbol_by_kinds};

/*
    The symbol kinds a scoped path may resolve through at each member:
    fields, states, parameters, variants and the mathematical/constant
    declarations — the same roster every path-walk step consults.
*/
const SCOPED_PATH_MEMBER_KINDS: &[SymbolKind] = &[
    SymbolKind::Field,
    SymbolKind::State,
    SymbolKind::Parameter,
    SymbolKind::Variant,
    SymbolKind::MathematicalDefinition,
    SymbolKind::Const,
];

/*
    Resolve a state-scoped table path to its lexical head and selected leaf.
    `indexed_last_member` carries the path's possible indexed tail
    (`items[index]`): one walk serves both, and only the final member's lookup
    consults the indexed declaration form.
*/
pub(super) fn resolve_state_scoped_table_path(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &crate::source_resolved::expression::ExpressionTable,
    path: &crate::source_resolved::expression::TableNamePath,
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

/*
    Retain each authored segment's declaration where it has one. Package-alias
    segments have zero identities; their complete prefix selects one exact
    declaration. An empty result means that selection or a descendant failed,
    so callers must not retain a partially selected suffix.
*/
pub(super) fn resolve_state_scoped_table_path_member_symbols(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &crate::source_resolved::expression::ExpressionTable,
    path: &crate::source_resolved::expression::TableNamePath,
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
    let mut resolved = vec![SymbolHandle::invalid(); prefix_length - 1];
    resolved.extend(suffix);

    if resolved.len() == members.len() {
        resolved
    } else {
        Default::default()
    }
}

/*
    A failed suffix does not invalidate the caller's independently selected
    lexical head, but it cannot publish a complete member-selection chain.
*/
pub(super) fn resolve_path_members_from_head(
    symbols: &SymbolTable,
    head: SymbolHandle,
    suffix: &[crate::source_resolved::name::DiagnosticName],
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

/*
    Resolve a run of authored path members to the lexical head and the selected
    leaf. `indexed_last_member` is the walk's only variation: when present, the
    final member's declaration is looked up in its indexed form (`name[index]`)
    instead of its plain form, and only at that final member.
*/
pub(super) fn resolve_state_scoped_members(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    members: &[crate::source_resolved::name::DiagnosticName],
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
    resolve_member_suffix(
        symbols,
        head,
        &members[prefix_length..],
        indexed_last_member,
    )
}

/// Resolve a receiver only against bindings preceding this occurrence.
/// State arenas also contain later locals; those are not lexical candidates.
pub(super) fn resolve_lexical_members(
    symbols: &SymbolTable,
    machine: &super::scopes::MachineScope<'_>,
    parameters: &[crate::source_resolved::signature::StateParameter],
    members: &[crate::source_resolved::name::DiagnosticName],
) -> (SymbolHandle, SymbolHandle) {
    let Some((first, suffix)) = members.split_first() else {
        return invalid_symbol_pair();
    };
    let head = super::expressions::lexical_value_head(
        machine,
        parameters,
        first.as_str(),
        !suffix.is_empty(),
    );
    if head.is_valid() {
        return resolve_member_suffix(symbols, head, suffix, None);
    }
    // Parameters were considered from the actual header above. Do not
    // reopen the whole state's local roster while trying a declaration.
    resolve_state_scoped_members(
        symbols,
        machine.symbol,
        SymbolHandle::invalid(),
        members,
        None,
    )
}

fn resolve_member_suffix(
    symbols: &SymbolTable,
    head: SymbolHandle,
    suffix: &[crate::source_resolved::name::DiagnosticName],
    indexed_last_member: Option<i64>,
) -> (SymbolHandle, SymbolHandle) {
    if !head.is_valid() {
        return invalid_symbol_pair();
    }
    let mut current = head;
    for (offset, member) in suffix.iter().enumerate() {
        let is_last = offset + 1 == suffix.len();
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

/*
    Lexical values own their suffix before static-address lookup. When the
    first segment is not a declaration (a loader-certified package alias),
    select the longest exact declaration prefix through ordinary name law.
    This is the same operation for a call receiver and an ordinary name path;
    aliases never become runtime values or grant visibility to another package.
*/
fn resolve_path_head(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    members: &[crate::source_resolved::name::DiagnosticName],
) -> (SymbolHandle, usize) {
    let (lookup, prefix_length) =
        resolve_path_head_lookup(symbols, machine_symbol, state_symbol, members);
    (lookup.unique().unwrap_or_default(), prefix_length)
}

fn resolve_path_head_lookup(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    members: &[crate::source_resolved::name::DiagnosticName],
) -> (SymbolLookup, usize) {
    match base::resolve_base_lookup(symbols, machine_symbol, state_symbol, &members[0]) {
        lookup @ (SymbolLookup::Unique(_) | SymbolLookup::Ambiguous { .. }) => {
            return (lookup, 1);
        }
        SymbolLookup::NotFound => {}
    }
    for prefix_length in (2..=members.len()).rev() {
        let prefix = &members[..prefix_length];
        let name = crate::source_resolved::name::DiagnosticName::new(
            prefix.joined_path(),
            super::lookup::diagnostic_path_source_span(prefix),
        );
        match base::resolve_base_lookup(symbols, machine_symbol, state_symbol, &name) {
            lookup @ (SymbolLookup::Unique(_) | SymbolLookup::Ambiguous { .. }) => {
                return (lookup, prefix_length);
            }
            SymbolLookup::NotFound => {}
        }
    }
    (SymbolLookup::NotFound, 1)
}

/// Preserve the ordinary lookup's absence or ambiguity while its exact source
/// and lexical frontier are available. A value's unresolved member needs type
/// selection; only a static namespace can establish a missing member here.
pub(in crate::resolution::names) fn unresolved_scoped_name(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    members: &[crate::source_resolved::name::DiagnosticName],
) -> crate::source_symbols::UnresolvedName {
    let Some(_) = members.first() else {
        return Default::default();
    };
    let (lookup, prefix_length) =
        resolve_path_head_lookup(symbols, machine_symbol, state_symbol, members);
    let SymbolLookup::Unique(head) = lookup else {
        return lookup.into();
    };
    if symbols.get(head).kind != SymbolKind::Module || prefix_length == members.len() {
        return Default::default();
    }
    let name = crate::source_resolved::name::DiagnosticName::new(
        members.joined_path(),
        super::lookup::diagnostic_path_source_span(members),
    );
    base::resolve_base_lookup(symbols, machine_symbol, state_symbol, &name).into()
}

pub(in crate::resolution::names) fn unresolved_scoped_call_name(
    symbols: &SymbolTable,
    owner: SymbolHandle,
    receiver: &[crate::source_resolved::name::DiagnosticName],
    target: &crate::source_resolved::name::DiagnosticName,
) -> crate::source_symbols::UnresolvedName {
    if receiver.is_empty() {
        return unresolved_scoped_name(
            symbols,
            owner,
            SymbolHandle::invalid(),
            std::slice::from_ref(target),
        );
    }
    let mut members = Vec::with_capacity(receiver.len() + 1);
    members.extend_from_slice(receiver);
    members.push(target.clone());
    unresolved_scoped_name(symbols, owner, SymbolHandle::invalid(), &members)
}

pub(super) fn invalid_symbol_pair() -> (SymbolHandle, SymbolHandle) {
    (SymbolHandle::invalid(), SymbolHandle::invalid())
}
