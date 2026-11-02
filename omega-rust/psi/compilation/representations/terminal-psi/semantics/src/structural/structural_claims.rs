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
    matches_paths(
        catalogue.iter().map(|binding| binding.path.as_slice()),
        returns,
        true,
    )
}

/// A public result signature permits each realization to return any admitted
/// case. Every unguarded claim is required on each exit; a realization need
/// not produce every possible guarded payload in the requirement's catalogue.
/// Actual case activity and claim completeness remain owned by return custody.
pub fn result_claim_paths_match_returns<'returns>(
    catalogue: &[Vec<StructuralPathSegment>],
    returns: impl IntoIterator<Item = &'returns [StructuralResultClaimBinding]>,
) -> bool {
    matches_paths(catalogue.iter().map(Vec::as_slice), returns, false)
}

fn matches_paths<'catalogue, 'returns>(
    catalogue: impl IntoIterator<Item = &'catalogue [StructuralPathSegment]>,
    returns: impl IntoIterator<Item = &'returns [StructuralResultClaimBinding]>,
    require_complete_catalogue: bool,
) -> bool {
    let mut catalogue_paths = BTreeSet::new();
    for path in catalogue {
        if !catalogue_paths.insert(path) {
            return false;
        }
    }
    let mut returned_paths = BTreeSet::new();
    let mut has_return = false;
    for bindings in returns {
        has_return = true;
        let paths = bindings
            .iter()
            .map(|binding| binding.path.as_slice())
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
    !require_complete_catalogue || !has_return || returned_paths == catalogue_paths
}
