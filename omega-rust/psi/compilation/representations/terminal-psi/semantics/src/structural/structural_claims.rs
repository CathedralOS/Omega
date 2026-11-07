//! Structural call results catalogue possible custody by typed path. A return
//! preserves the exact rows its source owns; different sum cases can therefore
//! contribute disjoint guarded rows to the same caller catalogue.
//!
//! This is signature geometry, not ownership replay. A Case path does not prove
//! its tag or mint a claim. The producer and consuming custody ledger must
//! independently establish each return's source, guards and complete live set.
use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeSet;
use terminal_psi::{
    StructuralPathSegment, StructuralResultClaimBinding, StructuralTypeDeclaration,
};

/// A value holds one case at a time, so its ownership record may list the
/// possible payloads without claiming that they coexist. Validate both paths
/// against the same structural type before comparing their ownership claims.
/// Reference crossings and runtime-selected storage cannot establish separate
/// owned paths. Borrowed storage keeps its physical overlap rule.
pub fn structural_claim_paths_may_overlap<'types>(
    declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    left: &[StructuralPathSegment],
    right: &[StructuralPathSegment],
) -> Option<bool> {
    crate::canonical_static_projection(declarations.clone(), root, left)?;
    crate::canonical_static_projection(declarations, root, right)?;
    Some(owned_paths_overlap(left, right))
}

/// Local result-row validation. Success establishes only typed possible slots
/// and return-path correspondence, never active case membership or live custody.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultClaimCatalogError {
    NonStaticOwnedPath,
    OverlappingPaths,
    ReturnPathMismatch,
}

/// Validate one exact result type's possible claim slots and every returning
/// occurrence. A guarded slot can remain unused when the callee returns other
/// declared cases. Each path must first be a static owned projection of this
/// root; a case spelling or storage interpretation cannot justify itself.
pub fn validate_result_claim_catalog<'types, 'returns>(
    declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    catalogue: &[StructuralResultClaimBinding],
    returns: impl IntoIterator<Item = &'returns [StructuralResultClaimBinding]>,
) -> Result<(), ResultClaimCatalogError> {
    crate::canonical_static_projection(declarations.clone(), root, &[])
        .ok_or(ResultClaimCatalogError::NonStaticOwnedPath)?;
    for binding in catalogue {
        crate::canonical_static_projection(declarations.clone(), root, &binding.path)
            .ok_or(ResultClaimCatalogError::NonStaticOwnedPath)?;
    }
    if catalogue.iter().enumerate().any(|(index, binding)| {
        catalogue[index + 1..]
            .iter()
            .any(|other| owned_paths_overlap(&binding.path, &other.path))
    }) {
        return Err(ResultClaimCatalogError::OverlappingPaths);
    }
    matches_paths(
        catalogue.iter().map(|binding| binding.path.as_slice()),
        returns,
    )
    .then_some(())
    .ok_or(ResultClaimCatalogError::ReturnPathMismatch)
}

fn owned_paths_overlap(left: &[StructuralPathSegment], right: &[StructuralPathSegment]) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

/// A public result signature permits each realization to return any admitted
/// case. Every unguarded claim is required on each exit; a realization need
/// not produce every possible guarded payload in the requirement's catalogue.
/// Actual case activity and claim completeness remain owned by return custody.
pub fn result_claim_paths_match_returns<'returns>(
    catalogue: &[Vec<StructuralPathSegment>],
    returns: impl IntoIterator<Item = &'returns [StructuralResultClaimBinding]>,
) -> bool {
    matches_paths(catalogue.iter().map(Vec::as_slice), returns)
}

fn matches_paths<'catalogue, 'returns>(
    catalogue: impl IntoIterator<Item = &'catalogue [StructuralPathSegment]>,
    returns: impl IntoIterator<Item = &'returns [StructuralResultClaimBinding]>,
) -> bool {
    let mut catalogue_paths = BTreeSet::new();
    for path in catalogue {
        if !catalogue_paths.insert(path) {
            return false;
        }
    }
    for bindings in returns {
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
    }
    true
}
