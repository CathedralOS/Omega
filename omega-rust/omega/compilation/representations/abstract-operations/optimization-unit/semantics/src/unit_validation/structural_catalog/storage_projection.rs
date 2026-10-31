//! Storage coordinates are not always standalone value endpoints. A byte window
//! keeps its enclosing backing type and exact interval; treating it as a fresh
//! array would lose custody or admit value copies that call admission forbids.
//! A byte field keeps its owning record: an inline owner's capacity and live
//! length, or a stored view's descriptor, have no standalone type identity
//! (`canonical_leaf_shape`), so the field coordinate itself is the storage a
//! call presentation borrows. Formation, bounds evidence and access remain the
//! owning operation's checks.

use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    ByteSequenceCarrier, ByteSequencePredicateSet, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape,
};

#[cfg(test)]
mod tests;

/// Whether `declaration` names a shared view: an element view or a borrowed
/// byte sequence. A value of such a type presents a loan of caller storage
/// rather than owned bytes, so every consumer that asks whether a place may be
/// copied, retired, or claimed as a mutable name counts both shapes alike.
pub(crate) fn shared_view_declaration(declaration: &StructuralTypeDeclaration) -> bool {
    declaration.shape.is_borrowed_byte_view()
        || matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuralStorageProjection {
    Endpoint(StructuralTypeId),
    ByteWindow {
        backing: StructuralTypeId,
        offset: u64,
        length: u64,
    },
    /// A byte field with no declared standalone endpoint, named through its
    /// owning record. It grants no reference formation or value copy; a call
    /// presents it only through the inline-owner or stored-view routes.
    ByteField {
        carrier: ByteSequenceCarrier,
        predicates: ByteSequencePredicateSet,
    },
}

impl StructuralStorageProjection {
    /// Rebinding preserves endpoint presentation, the backing and exact
    /// interval of a region, or a byte field's exact declared carrier and
    /// predicates. Neither equality nor presentation grants custody.
    pub fn matches<'types>(
        self,
        declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        expected: Self,
    ) -> bool {
        match (self, expected) {
            (Self::Endpoint(actual), Self::Endpoint(expected)) => {
                terminal_semantics::borrowed_referent_matches(declarations, actual, expected)
            }
            (
                Self::ByteWindow {
                    backing: actual,
                    offset,
                    length,
                },
                Self::ByteWindow {
                    backing: expected,
                    offset: expected_offset,
                    length: expected_length,
                },
            ) => {
                offset == expected_offset
                    && length == expected_length
                    && terminal_semantics::borrowed_referent_matches(declarations, actual, expected)
            }
            (actual @ Self::ByteField { .. }, expected @ Self::ByteField { .. }) => {
                actual == expected
            }
            _ => false,
        }
    }
}

pub fn structural_storage_projection<'types>(
    declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralStorageProjection> {
    if matches!(
        path.last(),
        Some(StructuralPathSegment::FixedByteRange { .. })
    ) {
        let window =
            terminal_semantics::fixed_byte_storage_window(declarations.clone(), root, path)?;
        Some(StructuralStorageProjection::ByteWindow {
            backing: super::paths::structural_projection_type(
                declarations,
                root,
                window.backing_path,
            )?,
            offset: window.offset,
            length: window.length,
        })
    } else {
        super::paths::structural_projection_type(declarations.clone(), root, path)
            .map(StructuralStorageProjection::Endpoint)
            .or_else(|| byte_field(declarations, root, path))
    }
}

/// A loan retains the same storage coordinate even when a restricted scalar
/// field presents only its read carrier. This is correspondence geometry, not
/// reference formation, liveness, or permission to write that field.
pub fn structural_borrowed_storage_projection<'types>(
    declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    access: terminal_psi::StructuralAccess,
) -> Option<StructuralStorageProjection> {
    if access != terminal_psi::StructuralAccess::Owned
        && let Some(endpoint) = terminal_semantics::borrowed_primitive_projection_declaration(
            declarations.clone(),
            root,
            path,
            None,
        )
    {
        return Some(StructuralStorageProjection::Endpoint(endpoint));
    }
    structural_storage_projection(declarations, root, path)
}

/// The byte field a path ends at, including a selected case payload's field.
/// A catalog declaration of the same carrier resolves first as an endpoint
/// above, so one catalog spells each field storage once.
fn byte_field<'types>(
    declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralStorageProjection> {
    let (StructuralPathSegment::Field(identity), container) = path.split_last()? else {
        return None;
    };
    let (container, case) = match container.split_last() {
        Some((StructuralPathSegment::Case(case), container)) => (container, Some(case)),
        _ => (container, None),
    };
    let owner = super::paths::structural_projection_type(declarations.clone(), root, container)?;
    let shape = &declarations
        .into_iter()
        .find(|declaration| declaration.id == owner)?
        .shape;
    let fields = match case {
        Some(case) => shape
            .sum_cases()?
            .iter()
            .find(|candidate| candidate.id == *case)?
            .fields
            .as_slice(),
        None => shape.direct_fields()?,
    };
    let field = terminal_psi::live_field_by_identity(fields, *identity)?;
    Some(StructuralStorageProjection::ByteField {
        carrier: field.field_type.byte_sequence_carrier()?,
        predicates: field.field_type.byte_predicates(),
    })
}
