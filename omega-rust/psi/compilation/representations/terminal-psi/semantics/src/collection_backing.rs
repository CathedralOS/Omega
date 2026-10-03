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

/// The element count the collection's view descriptor carries. A fixed
/// backing names it statically; an owned bounded byte carrier keeps it in
/// the aligned u64 word immediately before its inline bytes, where the
/// declared capacity only bounds the live count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionExtent {
    /// A compile-known element count fills the descriptor's extent word.
    Fixed(u64),
    /// The carrier's live element count fills the descriptor's extent word,
    /// loaded from the u64 header preceding `capacity` inline bytes.
    LiveLength { capacity: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectionBacking {
    pub extent: CollectionExtent,
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
            extent: CollectionExtent::Fixed(length),
            storage: CollectionStorage::Inline,
        }),
        StructuralTypeShape::ByteSequence(carrier)
            if matches!(
                carrier,
                ByteSequenceCarrier::BoundedView { .. } | ByteSequenceCarrier::BoundedOwned { .. }
            ) && types.clone().any(|declaration| {
                declaration.id == element
                    && matches!(declaration.shape,
                            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                                if integer.sign() == IntegerSign::Unsigned
                                    && integer.bits() == 8 && !integer.is_address())
            }) =>
        {
            let (ByteSequenceCarrier::BoundedView { capacity, .. }
            | ByteSequenceCarrier::BoundedOwned { capacity }) = carrier
            else {
                unreachable!("carrier is bounded by the guard")
            };
            Some(CollectionBacking {
                // A bounded view's declared capacity is its descriptor
                // extent. An owned carrier's live length sits in its header
                // word; capacity only bounds it.
                extent: match carrier {
                    ByteSequenceCarrier::BoundedOwned { .. } => {
                        CollectionExtent::LiveLength { capacity }
                    }
                    _ => CollectionExtent::Fixed(capacity),
                },
                // Owned backing holds its elements directly; a bounded view
                // is the referent pointer the descriptor carries instead.
                storage: match carrier {
                    ByteSequenceCarrier::BoundedOwned { .. } => CollectionStorage::Inline,
                    _ => CollectionStorage::Borrowed,
                },
            })
        }
        _ => None,
    }
}

/// Reconstruct a typed whole-collection loan. Exact element identity preserves
/// arbitrary established layouts; nonidentity needs the common leaf relation.
pub fn collection_view_backing<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    collection: StructuralTypeId,
    element: StructuralTypeId,
    access: terminal_psi::StructuralAccess,
) -> Option<CollectionBacking> {
    if let Some(backing) = collection_backing(types.clone(), collection, element) {
        return Some(backing);
    }
    let mut declarations = types.clone().filter(|row| row.id == collection);
    let declaration = declarations.next()?;
    if declarations.next().is_some() {
        return None;
    }
    let StructuralTypeShape::FixedArray {
        element: source,
        length,
    } = declaration.shape
    else {
        return None;
    };
    crate::element_representation_relation(types, source, element, access)?;
    Some(CollectionBacking {
        extent: CollectionExtent::Fixed(length),
        storage: CollectionStorage::Inline,
    })
}

/// Whether target element paths retain their source meaning without a leaf map.
/// Inexact geometry is conservative backing overlap, not indirect provenance.
pub fn collection_view_preserves_coordinates<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    collection: StructuralTypeId,
    element: StructuralTypeId,
    region: terminal_psi::ElementViewRegion,
) -> bool {
    matches!(region, terminal_psi::ElementViewRegion::WholeCollection)
        && collection_backing(types, collection, element).is_some()
}
