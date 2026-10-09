//! Structural call results catalogue possible custody by typed path. A return
//! preserves the exact rows its source owns; different sum cases can therefore
//! contribute disjoint guarded rows to the same caller catalogue.
//!
//! This is signature geometry, not ownership replay. A Case path does not prove
//! its tag or mint a claim. The producer and consuming custody ledger must
//! independently establish each return's source, guards and complete live set.
use std::collections::BTreeSet;
use terminal_psi::{StructuralPathSegment, StructuralResultClaimBinding};

/// Join machine-local return identities to the caller's possible path roster.
/// Every unguarded path occurs on every successful exit. Guarded paths occur
/// on the exits that own them, and their union is exactly the caller catalogue.
/// Neither a missing branch nor an extra potential payload can disappear in a
/// cardinality-only comparison.
pub fn result_claim_catalog_matches_returns<'returns>(
    catalogue: &[StructuralResultClaimBinding],
    returns: impl IntoIterator<Item = &'returns [StructuralResultClaimBinding]>,
) -> bool {
    let catalogue_paths = catalogue
        .iter()
        .map(|binding| &binding.path)
        .collect::<BTreeSet<_>>();
    if catalogue_paths.len() != catalogue.len() {
        return false;
    }
    let mut returned_paths = BTreeSet::new();
    let mut has_return = false;
    for bindings in returns {
        has_return = true;
        let paths = bindings
            .iter()
            .map(|binding| &binding.path)
            .collect::<BTreeSet<_>>();
        if paths.len() != bindings.len()
            || bindings
                .windows(2)
                .any(|pair| pair[0].claim >= pair[1].claim)
            || paths.iter().any(|path| !catalogue_paths.contains(path))
            || catalogue_paths.iter().any(|path| {
                !path
                    .iter()
                    .any(|segment| matches!(segment, StructuralPathSegment::Case(_)))
                    && !paths.contains(path)
            })
        {
            return false;
        }
        returned_paths.extend(paths);
    }
    !has_return || returned_paths == catalogue_paths
}
