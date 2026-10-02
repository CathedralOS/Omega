//! Conservative storage regions for observation and mutation replay.
//!
//! Paths name geometry, not authority. Callers independently authenticate the
//! root declaration and replay live custody. A runtime selector covers its
//! enclosing region; an unresolved field cannot establish disjointness.

use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralFieldId, StructuralTypeId};
use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape};

#[cfg(test)]
mod tests;

/// The enclosing direct projection and whether the access also crosses a
/// reference. Indirection is not a synthetic path segment: after a dynamic
/// selector that spelling could falsely prove two regions disjoint.
pub struct StorageRegion {
    pub path: Vec<StructuralPathSegment>,
    pub indirect: bool,
}

impl StorageRegion {
    /// Ordinary direct-root access cannot use an indirect path to prove a
    /// sibling disjoint. Reference-aware consumers instead resolve its custody.
    pub fn direct_path(&self) -> &[StructuralPathSegment] {
        if self.indirect { &[] } else { &self.path }
    }
}

/// Resolve a projection's static enclosing region, including its final field.
/// A reference crossing remains explicit and never denotes direct backing.
pub fn projected_storage_region<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: Option<StructuralFieldId>,
) -> StorageRegion {
    let static_length = path
        .iter()
        .position(|segment| {
            matches!(
                segment,
                StructuralPathSegment::RuntimeIndex { .. } | StructuralPathSegment::Referent
            )
        })
        .unwrap_or(path.len());
    let mut region = path[..static_length].to_vec();
    if static_length == path.len()
        && let Some(field) = field
        && let Some(declaration) = crate::runtime_structural_path_tip(declarations, root, &region)
        && let StructuralTypeShape::Record { fields } = &declaration.shape
        && let Some(field) = fields
            .iter()
            .find(|candidate| candidate.id == field && !candidate.relevance.is_erased())
    {
        region.push(StructuralPathSegment::Field(field.identity.clone()));
    }
    StorageRegion {
        path: region,
        indirect: path.contains(&StructuralPathSegment::Referent),
    }
}

/// Rejoin a canonical scalar-field observation to loan-origin path identities.
/// Unknown geometry covers the whole root, never an invented sibling.
pub fn canonical_field_storage_region<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
    field: StructuralFieldId,
) -> Vec<StructuralPathSegment> {
    let mut region = Vec::with_capacity(path.len() + 1);
    for (position, segment) in path
        .iter()
        .chain(std::iter::once(&CanonicalStructuralPathSegment::Field(
            field,
        )))
        .enumerate()
    {
        match segment {
            // Active-case custody is not established by an ordinary field read.
            CanonicalStructuralPathSegment::Case(_) => return Vec::new(),
            CanonicalStructuralPathSegment::FixedIndex(index) => {
                region.push(StructuralPathSegment::FixedIndex(*index))
            }
            CanonicalStructuralPathSegment::Field(field) => {
                let Some(declaration) = crate::canonical_structural_path_tip(
                    declarations.clone(),
                    root,
                    &path[..position],
                ) else {
                    return Vec::new();
                };
                let StructuralTypeShape::Record { fields } = &declaration.shape else {
                    return Vec::new();
                };
                let Some(field) = fields
                    .iter()
                    .find(|candidate| candidate.id == *field && !candidate.relevance.is_erased())
                else {
                    return Vec::new();
                };
                region.push(StructuralPathSegment::Field(field.identity.clone()));
            }
        }
    }
    region
}
