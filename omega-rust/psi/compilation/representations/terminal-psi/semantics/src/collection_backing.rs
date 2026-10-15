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
    /// A borrowed descriptor stores its captured count after its backing
    /// pointer. This is not an inline owner's live-length header.
    StoredLength,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectionBacking {
    pub extent: CollectionExtent,
    pub storage: CollectionStorage,
}

/// Geometry of the produced descriptor, independently of backing permission.
/// A fixed byte view keeps one pointer and obtains length from its declaration;
/// dynamic byte and element views store their length beside the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionViewDescriptor {
    StoredLength,
    DeclaredLength(u64),
}

/// Rejoin descriptor geometry to its declared whole backing. This grants no
/// access, initialization, index proof or lifetime. Fixed views cannot turn a
/// capacity bound on live storage, a ranged window, or representation bytes
/// into an exact array extent; dynamic views retain their existing judgments.
pub fn collection_view_descriptor(
    shape: &StructuralTypeShape,
    extent: Option<CollectionExtent>,
    region: terminal_psi::ElementViewRegion,
    source_path: &[terminal_psi::StructuralPathSegment],
    element: Option<&StructuralTypeDeclaration>,
) -> Option<CollectionViewDescriptor> {
    match shape {
        StructuralTypeShape::ElementView { .. }
        | StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. }) => {
            Some(CollectionViewDescriptor::StoredLength)
        }
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
            capacity, ..
        }) if extent == Some(CollectionExtent::Fixed(*capacity))
            && element.is_some_and(|declaration| {
                declaration.scalar_restriction.is_none()
                    && matches!(declaration.shape,
                    StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                        if integer.sign() == IntegerSign::Unsigned
                            && integer.bits() == 8 && !integer.is_address())
            })
            && region == terminal_psi::ElementViewRegion::WholeCollection
            && !matches!(
                source_path.last(),
                Some(terminal_psi::StructuralPathSegment::FixedByteRange { .. })
            ) =>
        {
            Some(CollectionViewDescriptor::DeclaredLength(*capacity))
        }
        _ => None,
    }
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
    collection_shape_backing(types, &declaration.shape, element)
}

fn collection_shape_backing<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    shape: &StructuralTypeShape,
    element: StructuralTypeId,
) -> Option<CollectionBacking> {
    match *shape {
        StructuralTypeShape::ElementView {
            element: actual, ..
        } if actual == element => Some(CollectionBacking {
            extent: CollectionExtent::StoredLength,
            storage: CollectionStorage::Borrowed,
        }),
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
    let mut declarations = types.clone().filter(|row| row.id == collection);
    let declaration = declarations.next()?;
    if declarations.next().is_some() {
        return None;
    }
    collection_shape_view_backing(types, &declaration.shape, element, access)
}

/// Inline collection fields carry geometry without a standalone type identity.
/// Use the selected shape directly; element relations still use exact identities.
pub(crate) fn collection_shape_view_backing<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    shape: &StructuralTypeShape,
    element: StructuralTypeId,
    access: terminal_psi::StructuralAccess,
) -> Option<CollectionBacking> {
    if let Some(backing) = collection_shape_backing(types.clone(), shape, element) {
        return Some(backing);
    }
    let StructuralTypeShape::FixedArray {
        element: source,
        length,
    } = *shape
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
