use crate::exactly_one::exactly_one;
use semantic_vocabulary::{IntegerSign, ScalarType, StructuralTypeId};
use terminal_psi::{ByteSequenceCarrier, StructuralTypeDeclaration, StructuralTypeShape};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionStorage {
    Inline,
    Borrowed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionExtent {
    Parameter {
        owner: semantic_vocabulary::MachineId,
        value: semantic_vocabulary::ValueId,
    },
    Fixed(u64),
    LiveLength {
        capacity: u64,
    },
    StoredLength,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectionBacking {
    pub extent: CollectionExtent,
    pub storage: CollectionStorage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionViewDescriptor {
    StoredLength,
    DeclaredLength(u64),
}

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
            capacity,
            ..
        }) if extent == Some(CollectionExtent::Fixed(*capacity))
            && element.is_some_and(|declaration| {
                declaration.scalar_restriction.is_none()
                    && matches!(declaration.shape,
                    StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                        if integer.sign() == IntegerSign::Unsigned
                            && integer.bits() == Some(8) && !integer.is_address())
            })
            && region == terminal_psi::ElementViewRegion::WholeCollection
            && !matches!(
                source_path.last(),
                Some(terminal_psi::StructuralPathSegment::FixedRange { .. })
            ) =>
        {
            Some(CollectionViewDescriptor::DeclaredLength(*capacity))
        }
        _ => None,
    }
}

pub fn collection_backing<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    collection: StructuralTypeId,
    element: StructuralTypeId,
) -> Option<CollectionBacking> {
    let declaration = crate::structural_declaration(types.clone(), collection)?;
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
            length: terminal_psi::ArrayExtent::Literal(length),
        } if actual == element => Some(CollectionBacking {
            extent: CollectionExtent::Fixed(length),
            storage: CollectionStorage::Inline,
        }),
        StructuralTypeShape::FixedArray {
            element: actual,
            length: terminal_psi::ArrayExtent::Parameter { owner, value },
        } if actual == element => Some(CollectionBacking {
            extent: CollectionExtent::Parameter { owner, value },
            storage: CollectionStorage::Inline,
        }),
        StructuralTypeShape::ByteSequence(carrier)
            if types.clone().any(|declaration| {
                declaration.id == element
                    && matches!(declaration.shape,
                            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                                if integer.sign() == IntegerSign::Unsigned
                                    && integer.bits() == Some(8) && !integer.is_address())
            }) =>
        {
            Some(CollectionBacking {
                extent: match carrier {
                    ByteSequenceCarrier::BoundedOwned { capacity } => {
                        CollectionExtent::LiveLength { capacity }
                    }
                    ByteSequenceCarrier::BoundedView { capacity, .. } => {
                        CollectionExtent::Fixed(capacity)
                    }
                    ByteSequenceCarrier::BorrowedView { .. } => CollectionExtent::StoredLength,
                },
                storage: match carrier {
                    ByteSequenceCarrier::BoundedOwned { .. } => CollectionStorage::Inline,
                    _ => CollectionStorage::Borrowed,
                },
            })
        }
        _ => None,
    }
}

pub fn collection_view_backing<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    collection: StructuralTypeId,
    element: StructuralTypeId,
    access: terminal_psi::StructuralAccess,
) -> Option<CollectionBacking> {
    let declaration = exactly_one(types.clone(), |row| row.id == collection)?;
    collection_shape_view_backing(types, &declaration.shape, element, access)
}

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
        length: terminal_psi::ArrayExtent::Literal(length),
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

pub fn collection_view_preserves_coordinates<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    collection: StructuralTypeId,
    element: StructuralTypeId,
    region: terminal_psi::ElementViewRegion,
) -> bool {
    matches!(region, terminal_psi::ElementViewRegion::WholeCollection)
        && collection_backing(types, collection, element).is_some()
}
