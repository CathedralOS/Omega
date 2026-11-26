use crate::source_resolved::name::DiagnosticName;
use crate::source_symbols::{SymbolLookup, SymbolTable};
use symbols::{BuiltinTypeAtom, SymbolHandle, SymbolKind};

/*
    The closed builtin type atom installed under `symbol_name`, or `None` for
    a name outside the roster. Every name-based builtin-type lookup reads the
    roster here instead of re-scanning `BuiltinTypeAtom::ALL`.
*/
pub(crate) fn builtin_type_atom_by_symbol_name(symbol_name: &str) -> Option<BuiltinTypeAtom> {
    BuiltinTypeAtom::ALL
        .into_iter()
        .find(|atom| atom.symbol_name() == symbol_name)
}

/*
    Search declaration scopes only. Enclosing invocation parameters, locals and
    live facts are never lexical declaration bindings. Stop at a shadowing cohort,
    including ambiguity; root/module visibility remains the ordinary lookup's job.
*/
pub(super) fn lexical_declaration_lookup(
    symbols: &SymbolTable,
    scope: SymbolHandle,
    name: &str,
    kinds: &[SymbolKind],
) -> SymbolLookup {
    symbols.lexical_declaration_lookup(scope, name, kinds)
}

pub(super) fn diagnostic_path_source_span(members: &[DiagnosticName]) -> source_text::SourceSpan {
    let Some(first) = members.first() else {
        return source_text::SourceSpan::default();
    };
    let Some(last) = members.last() else {
        return first.source_span();
    };
    if first.source_span().source_id != last.source_span().source_id {
        return first.source_span();
    }
    source_text::SourceSpan::new(
        first.source_span().source_id,
        source_text::Span::new(first.source_span().span.start, last.source_span().span.end),
    )
}

/*
    Whether an authored domain reference may select `domain` from
    `reference` under module name law. The exposure rules live on the symbol
    table (`SymbolTable::domain_name_reaches`) so every stage that pools or
    selects domain declarations applies one law: a fully qualified spelling
    always selects its exact declaration, while a relative spelling reaches a
    module-owned domain inside its own module or through an exposing import.
*/
pub(crate) fn domain_name_reaches(
    symbols: &SymbolTable,
    domain_symbol: SymbolHandle,
    _domain_name: &str,
    authored: &str,
    reference: source_text::SourceSpan,
) -> bool {
    symbols.domain_name_reaches(domain_symbol, authored, reference)
}

/*
    The type role among top-level declarations. A constant is a value and
    is never read here. A builtin type spelling names only the builtin
    type: no declaration may introduce a type with that spelling, and a
    same-spelled machine is a value, so it competes with nothing
    (`wiki/spec/language/modules.md#builtin-type-names`).
*/
pub(super) fn top_level_type_symbol_for_source(
    symbols: &SymbolTable,
    name: &DiagnosticName,
) -> SymbolHandle {
    symbols
        .lookup_source_type(name.as_str(), name.source_span())
        .unique()
        .unwrap_or_else(SymbolHandle::invalid)
}

pub(super) fn top_level_symbol_for_source(
    symbols: &SymbolTable,
    kind: SymbolKind,
    name: &DiagnosticName,
) -> SymbolHandle {
    symbols
        .find_top_level_by_name_and_kinds_from_source(name.as_str(), &[kind], name.source_span())
        .unwrap_or_else(SymbolHandle::invalid)
}

/*
    Select the carrier of a complete constructor name through ordinary source
    visibility. A full data name and a case-owner prefix are competing meanings,
    never alternatives selected by the number of path segments.
*/
pub(crate) fn constructor_type<'name>(
    symbols: &SymbolTable,
    name: &'name str,
    reference: source_text::SourceSpan,
) -> Result<Option<(SymbolHandle, Option<&'name str>)>, String> {
    let record = symbols.lookup_top_level_by_name_and_kinds_from_source_matching(
        name,
        &[SymbolKind::Data],
        reference,
        |_| true,
    );
    let (case, case_name) =
        name.rsplit_once("::")
            .map_or((SymbolLookup::NotFound, None), |(owner, case)| {
                (
                    symbols.lookup_top_level_by_name_and_kinds_from_source_matching(
                        owner,
                        &[SymbolKind::Data],
                        reference,
                        |symbol| {
                            child_symbol_by_kinds(symbols, symbol, &[SymbolKind::Variant], case)
                                .is_valid()
                        },
                    ),
                    Some(case),
                )
            });
    let case_symbol = |owner| {
        child_symbol_by_kinds(
            symbols,
            owner,
            &[SymbolKind::Variant],
            case_name.expect("eligible case owner retains its requested case"),
        )
    };
    let ambiguity =
        |first, second| selection_ambiguity(symbols, name, reference, first, second, "constructor");
    match (record, case) {
        (SymbolLookup::Ambiguous { first, second }, _) => Err(ambiguity(first, second)),
        (_, SymbolLookup::Ambiguous { first, second }) => {
            Err(ambiguity(case_symbol(first), case_symbol(second)))
        }
        (SymbolLookup::Unique(record), SymbolLookup::Unique(case)) => {
            Err(ambiguity(record, case_symbol(case)))
        }
        (SymbolLookup::Unique(symbol), SymbolLookup::NotFound) => Ok(Some((symbol, None))),
        (SymbolLookup::NotFound, SymbolLookup::Unique(symbol)) => Ok(Some((symbol, case_name))),
        (SymbolLookup::NotFound, SymbolLookup::NotFound) => Ok(None),
    }
}

fn selection_ambiguity(
    symbols: &SymbolTable,
    name: &str,
    reference: source_text::SourceSpan,
    first: SymbolHandle,
    second: SymbolHandle,
    role: &str,
) -> String {
    let imports = symbols
        .source_file_aliases(reference.source_id)
        .map(|alias| format!("|{alias}|"))
        .collect::<Vec<_>>();
    format!(
        "ambiguous {role} `{name}`: competing declarations `{}` and `{}`; source imports: {}",
        symbols.display_path(first, "::"),
        symbols.display_path(second, "::"),
        if imports.is_empty() {
            "(none)".to_owned()
        } else {
            imports.join(", ")
        },
    )
}

/// A value-position carrier-qualified case constructs a value only if its payload is empty.
pub(crate) fn qualified_payload_free_case_type<'name>(
    symbols: &SymbolTable,
    name: &'name str,
    reference: source_text::SourceSpan,
) -> Result<Option<(SymbolHandle, SymbolHandle, &'name str)>, String> {
    if !name.contains("::") {
        return Ok(None);
    }
    let Some((owner, Some(case))) = constructor_type(symbols, name, reference)? else {
        return Ok(None);
    };
    let case_symbol = child_symbol_by_kinds(symbols, owner, &[SymbolKind::Variant], case);
    if !case_symbol.is_valid()
        || symbols
            .child_handles(case_symbol)
            .is_none_or(|mut children| {
                children.any(|child| symbols.get(child).kind == SymbolKind::Field)
            })
    {
        return Ok(None);
    }
    Ok(Some((owner, case_symbol, case)))
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
    if parent == symbols.root() && symbols.get(parent).kind != SymbolKind::Module {
        return symbols
            .root_children_named(name)
            .find(|child| kinds.contains(&symbols.get(*child).kind))
            .unwrap_or_else(SymbolHandle::invalid);
    }
    child_symbol_by_kinds_matching(symbols, parent, kinds, name)
}

pub(super) fn child_or_attached_data_child_symbol_by_kinds(
    symbols: &SymbolTable,
    parent: SymbolHandle,
    kinds: &[SymbolKind],
    name: &str,
) -> SymbolHandle {
    let child = child_symbol_by_kinds(symbols, parent, kinds, name);
    if !child.is_valid() && symbols.get(parent).kind == SymbolKind::Variant {
        return child_symbol_by_kinds(symbols, symbols.get(parent).parent, kinds, name);
    }
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
    child_symbol_by_kinds_matching(symbols, parent, kinds, &indexed_member_name(name, index))
}

/*
    One spelling for an indexed member: the name, then the canonical decimal
    of the index. The receiver walk supplies that index from
    `IntegerLiteral::value_i64`, and `i64` displays it without a leading
    `+`, leading zeros, or spaces. Declaration names are identifiers, and
    the lexer keeps `[` a separate token, so no stored name is written in
    another bracket spelling.
*/
fn indexed_member_name(name: &str, index: i64) -> String {
    format!("{name}[{index}]")
}

pub(super) fn call_target_for_attached_data(
    symbols: &SymbolTable,
    attached_data: &str,
    target_name: &str,
    reference: source_text::SourceSpan,
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

/*
    Ask the symbol table once per requested kind. A private walk of the same
    children skipped source selection and, under a module, the kind, so one
    spelling was ambiguous here and a single declaration there.
*/
fn child_symbol_by_kinds_matching(
    symbols: &SymbolTable,
    parent: SymbolHandle,
    kinds: &[SymbolKind],
    name: &str,
) -> SymbolHandle {
    kinds
        .iter()
        .find_map(|kind| symbols.find_child_by_name_and_kind(parent, name, *kind))
        .unwrap_or_else(SymbolHandle::invalid)
}

/*
    Membership selects either a declared domain or an actual case of an exact
    carrier. Both meanings share the member namespace; ambiguity is not absence.
    Unlike construction, observing a case permits common fields and payloads.
*/
pub(crate) enum MembershipSelection {
    Domain(SymbolHandle),
    /// More than one attached domain is visible. Typing selects against the operand.
    DomainNeedsCarrier,
    Case {
        owner: SymbolHandle,
        case: SymbolHandle,
    },
}

/* The domain side of a membership selection. */
pub(crate) enum DomainMembership {
    NotFound,
    Unique(SymbolHandle),
    /// Typing uses the subject's applied carrier to select among these domains.
    Multiple {
        first: SymbolHandle,
    },
}

/// A file alias fixes the declaring file. Its domains can still have different
/// carriers. A bare name also sees the attached domains exposed to this source.
pub(crate) fn membership_domain_lookup(
    symbols: &SymbolTable,
    name: &str,
    reference: source_text::SourceSpan,
) -> DomainMembership {
    let mut candidates = match symbols.lookup_top_level_by_name_and_kinds_from_source_matching(
        name,
        &[SymbolKind::Domain],
        reference,
        |_| true,
    ) {
        SymbolLookup::Unique(symbol) => vec![symbol],
        SymbolLookup::Ambiguous { first, .. } => return DomainMembership::Multiple { first },
        SymbolLookup::NotFound => Vec::new(),
    };
    if !name.starts_with('|')
        && let Some(children) = symbols.child_handles(symbols.root())
    {
        for symbol in children {
            if symbols.get(symbol).kind != SymbolKind::Domain
                || candidates.contains(&symbol)
                || !symbols.source_reference_can_see_symbol(reference, symbol)
                || !domain_name_reaches(symbols, symbol, symbols.name(symbol), name, reference)
            {
                continue;
            }
            candidates.push(symbol);
        }
    }
    match candidates.as_slice() {
        [] => DomainMembership::NotFound,
        [symbol] => DomainMembership::Unique(*symbol),
        [first, ..] => DomainMembership::Multiple { first: *first },
    }
}

pub(crate) fn membership_selection(
    symbols: &SymbolTable,
    name: &str,
    reference: source_text::SourceSpan,
) -> Result<Option<MembershipSelection>, String> {
    let domain = membership_domain_lookup(symbols, name, reference);
    let (case_owner, case_name) =
        name.rsplit_once("::")
            .map_or((SymbolLookup::NotFound, ""), |(owner, case)| {
                (
                    symbols.lookup_top_level_by_name_and_kinds_from_source_matching(
                        owner,
                        &[SymbolKind::Data],
                        reference,
                        |owner| {
                            child_symbol_by_kinds(symbols, owner, &[SymbolKind::Variant], case)
                                .is_valid()
                        },
                    ),
                    case,
                )
            });
    let case = |owner| child_symbol_by_kinds(symbols, owner, &[SymbolKind::Variant], case_name);
    let ambiguous =
        |first, second| selection_ambiguity(symbols, name, reference, first, second, "membership");
    match (domain, case_owner) {
        (_, SymbolLookup::Ambiguous { first, second }) => Err(ambiguous(case(first), case(second))),
        (DomainMembership::Multiple { .. }, SymbolLookup::NotFound) => {
            Ok(Some(MembershipSelection::DomainNeedsCarrier))
        }
        (DomainMembership::Multiple { first, .. }, SymbolLookup::Unique(owner)) => {
            Err(ambiguous(first, case(owner)))
        }
        (DomainMembership::Unique(domain), SymbolLookup::Unique(owner)) => {
            Err(ambiguous(domain, case(owner)))
        }
        (DomainMembership::Unique(domain), SymbolLookup::NotFound) => {
            Ok(Some(MembershipSelection::Domain(domain)))
        }
        (DomainMembership::NotFound, SymbolLookup::Unique(owner)) => {
            Ok(Some(MembershipSelection::Case {
                owner,
                case: case(owner),
            }))
        }
        (DomainMembership::NotFound, SymbolLookup::NotFound) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use crate::source_resolved::name::DiagnosticName;
    use crate::source_symbols::SymbolTableBuilder;
    use numeric_literals::{IntegerLiteral, IntegerRadix};
    use symbols::{SymbolKind, SymbolNameRef};

    use super::super::paths::resolve_state_scoped_members;
    use super::child_indexed_symbol_by_kinds;

    /*
        A constant index selects the child whose name is the canonical
        `{member}[{i64}]` spelling. Source spellings `07` and `0_7` are the
        same i64 before that lookup; `+7` and ` 7 ` are not integer literals.
    */
    #[test]
    fn indexed_member_resolves_through_the_canonical_spelling() {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let machine = builder.insert_children(
            root,
            [(SymbolKind::Machine, SymbolNameRef::Borrowed("reader"))],
        );
        let machine = SymbolTableBuilder::child_handles(machine)
            .next()
            .expect("machine");
        let state = builder.insert_children(
            machine,
            [(SymbolKind::State, SymbolNameRef::Borrowed("entry"))],
        );
        let state = SymbolTableBuilder::child_handles(state)
            .next()
            .expect("state");
        let canonical = super::indexed_member_name("items", 7);
        let parameters = builder.insert_children(
            state,
            [
                (
                    SymbolKind::Parameter,
                    SymbolNameRef::Borrowed(canonical.as_str()),
                ),
                (SymbolKind::Parameter, SymbolNameRef::Borrowed("items[07]")),
            ],
        );
        let mut parameters = SymbolTableBuilder::child_handles(parameters);
        let canonical_symbol = parameters.next().expect("canonical parameter");
        let padded_symbol = parameters.next().expect("padded parameter");
        let symbols = builder.finish();

        let found =
            child_indexed_symbol_by_kinds(&symbols, state, &[SymbolKind::Parameter], "items", 7);
        assert_eq!(found, canonical_symbol);
        assert_ne!(found, padded_symbol);
        assert_eq!(symbols.name(found), "items[7]");

        let (head, leaf) = resolve_state_scoped_members(
            &symbols,
            machine,
            state,
            &[DiagnosticName::generated("items")],
            Some(7),
        );
        assert_eq!(head, canonical_symbol);
        assert_eq!(leaf, canonical_symbol);

        for spelling in ["7", "07", "007", "0_7"] {
            let literal =
                IntegerLiteral::from_parts(false, IntegerRadix::Decimal, spelling).unwrap();
            let index = literal.value_i64().expect(spelling);
            assert_eq!(index, 7, "{spelling}");
            assert_eq!(
                child_indexed_symbol_by_kinds(
                    &symbols,
                    state,
                    &[SymbolKind::Parameter],
                    "items",
                    index,
                ),
                canonical_symbol,
                "{spelling}",
            );
        }
        assert!(IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "+7").is_err());
        assert!(IntegerLiteral::from_parts(false, IntegerRadix::Decimal, " 7 ").is_err());
    }
}
