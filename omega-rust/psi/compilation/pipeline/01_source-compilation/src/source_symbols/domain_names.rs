//! One source-scoped domain lookup for memberships and qualified declarations.

use super::{SymbolLookup, SymbolTable};
use symbols::{SymbolHandle, SymbolKind};

/* The domain side of a membership selection. */
pub enum DomainMembership {
    NotFound,
    Unique(SymbolHandle),
    /*
        The ordinary path law found competing exact selections — an authored
        ambiguity that rejects immediately.
    */
    Ambiguous {
        first: SymbolHandle,
        second: SymbolHandle,
    },
    /*
        The attached-domain exposure pool claims the spelling under more than
        one distinct owner. The shared membership lookup declines that pool
        here rather than reporting ambiguity: downstream validation owns the
        diagnostic, and no other meaning may claim the contested spelling.
    */
    Contested,
}

/*
    Pool every domain candidate an authored membership name can select: the
    ordinary qualified/import path lookup unioned with the attached-domain
    exposure law (`domain_name_reaches`), which admits a carrier-qualified or
    leaf spelling through a declaring-module or exact-domain import.
    Module-local owners outrank every other tier. The same symbol found by
    both routes is one candidate. Two different symbols stay contested even
    when their display paths match. Expression membership, proof-fact
    membership, and token homes all read this pool.
*/
pub fn membership_domain_lookup(
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
                || !symbols.domain_name_reaches(symbol, name, reference)
            {
                continue;
            }
            candidates.push(symbol);
        }
    }
    let mut distinct = Vec::new();
    for candidate in candidates {
        if !distinct.contains(&candidate) {
            distinct.push(candidate);
        }
    }
    match distinct.as_slice() {
        [] => DomainMembership::NotFound,
        [symbol] => DomainMembership::Unique(*symbol),
        _ => DomainMembership::Contested,
    }
}
