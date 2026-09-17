use symbols::{SymbolHandle, SymbolKind, SymbolLookup, SymbolTable};

pub(super) fn diagnostic_path_source_span(
    members: &[symbol_resolved_trees::name::DiagnosticName],
) -> source::SourceSpan {
    let Some(first) = members.first() else {
        return source::SourceSpan::default();
    };
    let Some(last) = members.last() else {
        return first.source_span();
    };
    if first.source_span().source_id != last.source_span().source_id {
        return first.source_span();
    }
    source::SourceSpan::new(
        first.source_span().source_id,
        source::Span::new(first.source_span().span.start, last.source_span().span.end),
    )
}

/// Whether an authored domain reference may select `domain` from
/// `reference` under module name law. The exposure rules live on the symbol
/// table (`SymbolTable::domain_name_reaches`) so every stage that pools or
/// selects domain declarations applies one law: a fully qualified spelling
/// always selects its exact declaration, while a relative spelling reaches a
/// module-owned domain inside its own module or through an exposing import.
pub(crate) fn domain_name_reaches(
    symbols: &SymbolTable,
    domain_symbol: SymbolHandle,
    _domain_name: &str,
    authored: &str,
    reference: source::SourceSpan,
) -> bool {
    symbols.domain_name_reaches(domain_symbol, authored, reference)
}

/// A domain declared in the reference's own module outranks same-spelled
/// foreign or root candidates, mirroring local-binding precedence.
pub(crate) fn prefer_module_local_domain(
    symbols: &SymbolTable,
    candidates: Vec<SymbolHandle>,
    reference: source::SourceSpan,
) -> Vec<SymbolHandle> {
    if reference.span.start == reference.span.end {
        return candidates;
    }
    let reference_module = symbols.source_module(reference.source_id);
    if !reference_module.is_valid() {
        return candidates;
    }
    let local = candidates
        .iter()
        .copied()
        .filter(|candidate| symbols.symbol_module(*candidate) == reference_module)
        .collect::<Vec<_>>();
    if local.is_empty() { candidates } else { local }
}

pub(super) fn top_level_type_symbol_for_source(
    symbols: &SymbolTable,
    name: &symbol_resolved_trees::name::DiagnosticName,
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
    name: &symbol_resolved_trees::name::DiagnosticName,
) -> SymbolHandle {
    symbols
        .find_top_level_by_name_and_kinds_from_source(name.as_str(), &[kind], name.source_span())
        .unwrap_or_else(SymbolHandle::invalid)
}

/// Select the carrier of a complete constructor name through ordinary source
/// visibility. A full data name and a case-owner prefix are competing meanings,
/// never alternatives selected by the number of path segments.
pub(crate) fn constructor_type<'name>(
    symbols: &SymbolTable,
    name: &'name str,
    reference: source::SourceSpan,
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
    reference: source::SourceSpan,
    first: SymbolHandle,
    second: SymbolHandle,
    role: &str,
) -> String {
    let imports = symbols
        .source_module_import_paths(reference.source_id)
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

/// A bare value may construct only a payload-free case. The caller resolves
/// lexical binders first; named constant prefixes remain values, including an
/// ambiguous prefix that must never be reinterpreted as a namespace.
pub(crate) fn bare_case_type<'name>(
    symbols: &SymbolTable,
    name: &'name str,
    reference: source::SourceSpan,
) -> Result<Option<(SymbolHandle, SymbolHandle, &'name str)>, String> {
    let Some((prefix, _)) = name.split_once("::") else {
        return Ok(None);
    };
    match symbols.lookup_top_level_by_name_and_kinds_from_source_matching(
        prefix,
        &[SymbolKind::Const],
        reference,
        |_| true,
    ) {
        SymbolLookup::Unique(_) => return Ok(None),
        SymbolLookup::Ambiguous { first, second } => {
            return Err(selection_ambiguity(
                symbols,
                name,
                reference,
                first,
                second,
                "constructor",
            ));
        }
        SymbolLookup::NotFound => {}
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
    child_symbol_by_kinds_matching(symbols, parent, kinds, |symbol_name| symbol_name == name)
}

pub(super) fn child_or_attached_data_child_symbol_by_kinds(
    symbols: &SymbolTable,
    parent: SymbolHandle,
    kinds: &[SymbolKind],
    name: &str,
) -> SymbolHandle {
    let child = child_symbol_by_kinds(symbols, parent, kinds, name);
    if !child.is_valid() && symbols.get(parent).kind == SymbolKind::Variant {
        // A case payload has its own lexical fields, then inherits its data's
        // common fields. Follow declaration identity, not the case's spelling.
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
    child_symbol_by_kinds_matching(symbols, parent, kinds, |symbol_name| {
        symbol_name_matches_indexed_member(symbol_name, name, index)
    })
}

pub(super) fn call_target_for_attached_data(
    symbols: &SymbolTable,
    attached_data: &str,
    target_name: &str,
    reference: source::SourceSpan,
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

    let mut matched = SymbolHandle::invalid();
    for child in children {
        let symbol = symbols.get(child);
        if matches_name(symbols.name(child))
            && (kinds.contains(&symbol.kind) || symbols.get(parent).kind == SymbolKind::Module)
        {
            if symbols.get(parent).kind != SymbolKind::Module {
                return child;
            }
            if matched.is_valid() {
                return SymbolHandle::invalid();
            }
            matched = child;
        }
    }

    matched
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

/// Membership selects either a declared domain or an actual case of an exact
/// carrier. Both meanings share the member namespace; ambiguity is not absence.
/// Unlike construction, observing a case permits common fields and payloads.
pub(crate) enum MembershipSelection {
    Domain(SymbolHandle),
    Case {
        owner: SymbolHandle,
        case: SymbolHandle,
    },
}

/// The domain side of a membership selection.
enum DomainMembership {
    NotFound,
    Unique(SymbolHandle),
    /// The ordinary path law found competing exact selections — an authored
    /// ambiguity that rejects immediately.
    Ambiguous {
        first: SymbolHandle,
        second: SymbolHandle,
    },
    /// The attached-domain exposure pool claims the spelling under more than
    /// one distinct owner. Like the proof-fact resolver's pooled decline,
    /// this retains the membership as a checked obligation rather than
    /// reporting ambiguity here: downstream validation owns the diagnostic,
    /// and no other meaning may claim the contested spelling.
    Contested,
}

/// Pool every domain candidate an authored membership name can select: the
/// ordinary qualified/import path lookup unioned with the attached-domain
/// exposure law (`domain_name_reaches`), which admits a carrier-qualified or
/// leaf spelling through a declaring-module or exact-domain import.
/// Module-local owners outrank every other tier, and declarations sharing one
/// complete logical path are one semantic candidate.
fn membership_domain_lookup(
    symbols: &SymbolTable,
    name: &str,
    reference: source::SourceSpan,
) -> DomainMembership {
    let mut candidates = match symbols.lookup_top_level_by_name_and_kinds_from_source_matching(
        name,
        &[SymbolKind::Domain],
        reference,
        |_| true,
    ) {
        SymbolLookup::Unique(symbol) => vec![symbol],
        SymbolLookup::Ambiguous { first, second } => {
            return DomainMembership::Ambiguous { first, second };
        }
        SymbolLookup::NotFound => Vec::new(),
    };
    if let Some(children) = symbols.child_handles(symbols.root()) {
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
    let candidates = prefer_module_local_domain(symbols, candidates, reference);
    let mut unique: Vec<(String, SymbolHandle)> = Vec::new();
    'next: for candidate in candidates {
        let path = symbols.display_path(candidate, "::");
        for (existing, _) in &unique {
            if *existing == path {
                continue 'next;
            }
        }
        unique.push((path, candidate));
    }
    match unique.as_slice() {
        [] => DomainMembership::NotFound,
        [(_, symbol)] => DomainMembership::Unique(*symbol),
        _ => DomainMembership::Contested,
    }
}

pub(crate) fn membership_selection(
    symbols: &SymbolTable,
    name: &str,
    reference: source::SourceSpan,
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
        (DomainMembership::Ambiguous { first, second }, _) => Err(ambiguous(first, second)),
        (_, SymbolLookup::Ambiguous { first, second }) => Err(ambiguous(case(first), case(second))),
        // A contested domain claim is not absence: the case side must not win
        // a spelling the exposure pool cannot settle, and the membership keeps
        // its checked obligation instead of reporting the contest here.
        (DomainMembership::Contested, _) => Ok(None),
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
mod membership_tests;

#[cfg(test)]
mod constructor_tests {
    use crate::symbols::constructor_type;
    use symbols::SymbolKind;
    use symbols::{SymbolNameRef, SymbolTableBuilder};

    #[test]
    fn only_an_actual_case_competes_with_a_complete_record_name() {
        for has_case in [false, true] {
            let mut builder = SymbolTableBuilder::new();
            let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let children = builder.insert_children(
                root,
                [
                    (SymbolKind::Data, SymbolNameRef::Static("Owner")),
                    (SymbolKind::Data, SymbolNameRef::Static("Owner::Value")),
                ],
            );
            let mut handles = SymbolTableBuilder::child_handles(children);
            let owner = handles.next().unwrap();
            let record = handles.next().unwrap();
            builder.insert_children(
                owner,
                [(
                    if has_case {
                        SymbolKind::Variant
                    } else {
                        SymbolKind::Field
                    },
                    SymbolNameRef::Static("Value"),
                )],
            );
            let symbols = builder.finish();
            let selected =
                constructor_type(&symbols, "Owner::Value", source::SourceSpan::default());
            if has_case {
                assert!(
                    selected.is_err(),
                    "both actual meanings must remain ambiguous"
                );
            } else {
                assert_eq!(selected, Ok(Some((record, None))));
            }
        }
    }
}
