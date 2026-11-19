use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeSet;
use terminal_psi::{
    StructuralPathSegment, StructuralResultClaimBinding, StructuralTypeDeclaration,
};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultClaimCatalogError {
    NonStaticOwnedPath,
    OverlappingPaths,
    ReturnPathMismatch,
    TypedFrontierMismatch,
}

pub fn validate_result_claim_catalog<'types, 'returns>(
    declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    catalogue: &[StructuralResultClaimBinding],
    returns: impl IntoIterator<Item = &'returns [StructuralResultClaimBinding]>,
) -> Result<(), ResultClaimCatalogError> {
    let mut declarations = declarations.collect::<Vec<_>>();
    declarations.sort_unstable_by_key(|row| row.id);
    if declarations.windows(2).any(|rows| rows[0].id == rows[1].id) {
        return Err(ResultClaimCatalogError::NonStaticOwnedPath);
    }
    crate::canonical_static_projection(declarations.iter().copied(), root, &[])
        .ok_or(ResultClaimCatalogError::NonStaticOwnedPath)?;
    for binding in catalogue {
        crate::canonical_static_projection(declarations.iter().copied(), root, &binding.path)
            .ok_or(ResultClaimCatalogError::NonStaticOwnedPath)?;
    }
    if catalogue.iter().enumerate().any(|(index, binding)| {
        catalogue[index + 1..]
            .iter()
            .any(|other| owned_paths_overlap(&binding.path, &other.path))
    }) {
        return Err(ResultClaimCatalogError::OverlappingPaths);
    }
    // This is the guarded type catalogue, not evidence of a selected live
    // case. Every returning edge still proves its own active membership.
    let claim_types = super::owned_structure::ClaimTypes::new(declarations.iter().copied())
        .map_err(|_| ResultClaimCatalogError::TypedFrontierMismatch)?;
    let expected = super::owned_structure::claim_catalogue(&claim_types, root, catalogue.len())
        .map_err(|_| ResultClaimCatalogError::TypedFrontierMismatch)?;
    if expected.len() != catalogue.len()
        || catalogue
            .iter()
            .any(|binding| !expected.contains(&binding.path))
    {
        return Err(ResultClaimCatalogError::TypedFrontierMismatch);
    }
    matches_paths(
        catalogue.iter().map(|binding| binding.path.as_slice()),
        returns,
    )
    .then_some(())
    .ok_or(ResultClaimCatalogError::ReturnPathMismatch)
}

fn owned_paths_overlap(
    left: &[StructuralPathSegment],
    right: &[StructuralPathSegment],
) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

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
