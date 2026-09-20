//! Exact static `Field`/`FixedIndex` path resolution shared by the semantic
//! rows.
//!
//! One rule: each hop descends through the declared type catalog — a `Field`
//! segment names a non-erased record field holding a structural child, and a
//! `FixedIndex` segment stays inside a fixed array's declared extent. The
//! resolved tip is evidence, not authority: access, multiplicity,
//! qualification, custody, and claim obligations stay with each caller.
//!
//! Two entry points mirror the two segment vocabularies. The canonical form
//! resolves declaration-local field ids and rejects a duplicated declaration
//! or field id rather than picking one; the runtime form resolves field
//! identities and takes the first match, as the validated catalog admits no
//! duplicates. Neither grants the place any storage, view adapter, or type
//! identity of its own.

use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralTypeId};
use terminal_psi::{
    StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

/// Resolve `path` from `root` to its tip declaration, rejecting a duplicated
/// declaration or field id. This grants no access or establishment authority
/// for the root.
pub fn canonical_structural_path_tip<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    mut root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<&'a StructuralTypeDeclaration> {
    for segment in path {
        let mut matching = declarations
            .clone()
            .filter(|declaration| declaration.id == root);
        let declaration = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        root = match (segment, &declaration.shape) {
            (
                CanonicalStructuralPathSegment::Field(identity),
                StructuralTypeShape::Record { fields },
            ) => {
                let mut matching = fields.iter().filter(|field| field.id == *identity);
                let field = matching.next()?;
                if matching.next().is_some() || field.relevance.is_erased() {
                    return None;
                }
                let StructuralFieldType::Structural(child) = field.field_type else {
                    return None;
                };
                child
            }
            (
                CanonicalStructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            _ => return None,
        };
    }
    let mut matching = declarations.filter(|declaration| declaration.id == root);
    let declaration = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    Some(declaration)
}

/// Resolve `path` from `root` to its tip declaration over runtime field
/// identities, taking the first match the validated catalog presents. This
/// grants no access or establishment authority for the root.
pub fn runtime_structural_path_tip<'a>(
    mut types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    mut root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<&'a StructuralTypeDeclaration> {
    for segment in path {
        let declaration = types.clone().find(|declaration| declaration.id == root)?;
        root = match (segment, &declaration.shape) {
            (StructuralPathSegment::Field(identity), StructuralTypeShape::Record { fields }) => {
                let field = fields
                    .iter()
                    .find(|field| field.identity == *identity && !field.relevance.is_erased())?;
                let StructuralFieldType::Structural(next) = field.field_type else {
                    return None;
                };
                next
            }
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            _ => return None,
        };
    }
    types.find(|declaration| declaration.id == root)
}
