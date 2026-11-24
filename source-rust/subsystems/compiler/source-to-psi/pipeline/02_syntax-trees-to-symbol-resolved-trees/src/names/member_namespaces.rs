/*
    One member namespace per data declaration.

    `wiki/spec/language/data_and_literals.md`, "Records, sums and nested
    declarations": "Cases, nested data and attached domains share their
    owner's member namespace; data does not own machine declarations.
    Collisions reject rather than selecting by priority." A case also declares
    its same-named domain, so `domain Signal::Ready;` beside `case Ready;`
    names one member twice, and so does a second `case Ready;`.

    The check reads the resolved declarations once: every case of a selected
    data declaration and every selected domain attached to one, sorted by
    owner and name, then compared with its neighbour. Fields are not members
    of this namespace. A nested data declaration joins as its owner's member.
*/

use diagnostics::Diagnostic;
use source_text::SourceSpan;
use symbols::SymbolHandle;

use crate::source_resolved::SymbolResolvedTrees;
use crate::source_resolved::data::DataMember;
use crate::source_resolved::types::TypeReference;

#[derive(Clone, Copy, PartialEq, Eq)]
enum MemberKind {
    Case,
    NestedData,
    Domain,
}

impl MemberKind {
    const fn named(self) -> &'static str {
        match self {
            Self::Case => "case",
            Self::NestedData => "nested data declaration",
            Self::Domain => "attached domain",
        }
    }
}

struct Member<'program> {
    owner: SymbolHandle,
    name: &'program str,
    kind: MemberKind,
    span: SourceSpan,
}

/// The data declaration a domain attaches to, when its carrier names one.
fn attached_owner(program: &SymbolResolvedTrees, carrier: &TypeReference) -> Option<SymbolHandle> {
    let symbol = match carrier {
        TypeReference::Named { symbol, .. } => *symbol,
        TypeReference::Generic(application) => application.base_symbol,
        _ => return None,
    };
    (symbol.is_valid()
        && program
            .data_definitions
            .iter()
            .any(|definition| definition.symbol == symbol))
    .then_some(symbol)
}

pub(crate) fn reject_member_collisions(
    program: &SymbolResolvedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let selected = |symbol| program.symbols.symbol_source_is_selected(symbol);
    let mut members = Vec::new();
    for definition in program
        .data_definitions
        .iter()
        .filter(|definition| selected(definition.symbol))
    {
        for member in program.data_members(definition.members) {
            if let DataMember::Variant(variant) = member {
                members.push(Member {
                    owner: definition.symbol,
                    name: variant.name.as_str(),
                    kind: MemberKind::Case,
                    span: variant.name.source_span(),
                });
            }
        }
    }
    // The parser names a nested declaration `Owner::Name` beside its owner
    // in the same file; the member is its last segment.
    for nested in program
        .data_definitions
        .iter()
        .filter(|definition| selected(definition.symbol))
    {
        let path = nested.name.as_str();
        let Some((owner_path, name)) = path.rsplit_once("::") else {
            continue;
        };
        let span = nested.name.source_span();
        let Some(owner) = program.data_definitions.iter().find(|owner| {
            owner.name.as_str() == owner_path
                && owner.name.source_span().source_id == span.source_id
        }) else {
            continue;
        };
        members.push(Member {
            owner: owner.symbol,
            name,
            kind: MemberKind::NestedData,
            span,
        });
    }
    for domain in program
        .domain_definitions
        .iter()
        .filter(|domain| selected(domain.symbol) && domain.alias.is_none())
    {
        if let Some(owner) = attached_owner(program, &domain.target_type) {
            // A domain's name is its whole path, `Signal::Ready`; the member
            // is its last segment.
            let path = domain.name.as_str();
            members.push(Member {
                owner,
                name: path.rsplit("::").next().unwrap_or(path),
                kind: MemberKind::Domain,
                span: domain.name.source_span(),
            });
        }
    }
    // Source order within one name keeps the earlier declaration first.
    members.sort_by(|left, right| {
        (left.owner.arena_index(), left.name, left.span.span.start).cmp(&(
            right.owner.arena_index(),
            right.name,
            right.span.span.start,
        ))
    });
    let diagnostics = members
        .windows(2)
        .filter(|pair| pair[0].owner == pair[1].owner && pair[0].name == pair[1].name)
        .map(|pair| {
            let (first, second) = (&pair[0], &pair[1]);
            Diagnostic::error(format!(
                "`{}::{}` is declared twice: this {} and an earlier {} share the data \
                 declaration's one member namespace, and a collision rejects",
                program.symbols.display_path(second.owner, "::"),
                second.name,
                second.kind.named(),
                first.kind.named(),
            ))
            .with_source_span(second.span)
        })
        .collect::<Vec<_>>();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
