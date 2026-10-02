//! Declared extent and residence of contiguous collection backing.
//!
//! A bounded byte reference carries a pointer, not its referent's bytes or a
//! runtime-length descriptor. Its declared capacity still names the entire
//! borrowed array. Keeping residence separate from extent lets establishment
//! use the same operation for inline arrays and already borrowed backing.
//! This query grants no access, initialization, bounds proof or lifetime.

use semantic_vocabulary::{IntegerSign, ScalarType, StructuralTypeId};
use terminal_psi::{ByteSequenceCarrier, StructuralTypeDeclaration, StructuralTypeShape};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionStorage {
    /// The declared collection contains its elements directly.
    Inline,
    /// The declared collection contains a pointer to borrowed elements.
    Borrowed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectionBacking {
    pub length: u64,
    pub storage: CollectionStorage,
}

/// Match a collection to the establishment's exact element declaration.
/// Bounded byte backing has implicit primitive `u8` elements; another scalar
/// with the same width, or an owned/dynamic byte carrier, cannot substitute.
pub fn collection_backing<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    collection: StructuralTypeId,
    element: StructuralTypeId,
) -> Option<CollectionBacking> {
    let declaration = types
        .clone()
        .find(|declaration| declaration.id == collection)?;
    match declaration.shape {
        StructuralTypeShape::FixedArray {
            element: actual,
            length,
        } if actual == element => Some(CollectionBacking {
            length,
            storage: CollectionStorage::Inline,
        }),
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
            capacity, ..
        }) if types.clone().any(|declaration| {
            declaration.id == element
                && matches!(declaration.shape,
                        StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                            if integer.sign() == IntegerSign::Unsigned
                                && integer.bits() == 8 && !integer.is_address())
        }) =>
        {
            Some(CollectionBacking {
                length: capacity,
                storage: CollectionStorage::Borrowed,
            })
        }
        _ => None,
    }
}
