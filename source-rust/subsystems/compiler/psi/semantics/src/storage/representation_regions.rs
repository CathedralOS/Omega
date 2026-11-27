use crate::exactly_one::exactly_one;
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType,
    StructuralTypeId, ValueId,
};
use terminal_psi::{
    ByteSequenceCarrier, ElementViewRegion, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepresentationScalarLeaf {
    pub path: Vec<StructuralPathSegment>,
    pub offset: u64,
    pub scalar_type: ScalarType,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepresentationElementLayout {
    pub size: u64,
    pub alignment: u64,
    pub leaves: Vec<RepresentationScalarLeaf>,
}

impl RepresentationElementLayout {
    pub(crate) fn is_tightly_tiled(&self) -> bool {
        let mut end = 0;
        for leaf in &self.leaves {
            if leaf.offset != end {
                return false;
            }
            let Some(width) =
                crate::storage::element_representation::scalar_size(leaf.scalar_type)
            else {
                return false;
            };
            let Some(next) = end.checked_add(width) else {
                return false;
            };
            end = next;
        }
        end == self.size
    }
}

pub fn representation_element_layout<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    element: StructuralTypeId,
) -> Option<RepresentationElementLayout> {
    let layout = crate::element_representation_layout(types, element)?;
    if layout
        .leaves
        .iter()
        .any(|leaf| leaf.bounds.is_some() || leaf.scalar_type == ScalarType::Boolean)
    {
        return None;
    }
    Some(RepresentationElementLayout {
        size: layout.size,
        alignment: layout.alignment,
        leaves: layout
            .leaves
            .into_iter()
            .map(|leaf| RepresentationScalarLeaf {
                path: leaf.path,
                offset: leaf.offset,
                scalar_type: leaf.scalar_type,
            })
            .collect(),
    })
}

pub fn representation_byte_extent<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    collection: StructuralTypeId,
) -> Option<u64> {
    let row = exactly_one(types.clone(), |row| row.id == collection)?;
    representation_shape_byte_extent(types, &row.shape)
}

fn representation_shape_byte_extent<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    shape: &StructuralTypeShape,
) -> Option<u64> {
    match shape {
        StructuralTypeShape::FixedArray {
            element,
            length: terminal_psi::ArrayExtent::Literal(length),
        } => {
            let element = exactly_one(types, |row| row.id == *element)?;
            matches!(element.shape, StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)) if !integer.is_address() && integer.sign() == IntegerSign::Unsigned && integer.bits()==Some(8)).then_some(*length)
        }
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
            capacity,
            ..
        }) => Some(*capacity),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub enum ElementViewRegionGeometry {
    WholeCollection(crate::CollectionBacking),
    WholeRepresentation {
        backing: crate::CollectionBacking,
        source: RepresentationElementLayout,
        target: RepresentationElementLayout,
    },
    RepresentationBytes {
        byte_extent: u64,
        element: RepresentationElementLayout,
    },
}
pub fn element_view_shape_region_geometry<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    shape: &StructuralTypeShape,
    element: StructuralTypeId,
    region: ElementViewRegion,
    access: terminal_psi::StructuralAccess,
) -> Option<ElementViewRegionGeometry> {
    match region {
        ElementViewRegion::WholeCollection => {
            crate::storage::collection_backing::collection_shape_view_backing(
                types, shape, element, access,
            )
            .map(ElementViewRegionGeometry::WholeCollection)
        }
        ElementViewRegion::WholeRepresentation { .. } => {
            if access != terminal_psi::StructuralAccess::SharedBorrow {
                return None;
            }
            let source_element = match shape {
                StructuralTypeShape::ElementView { element, .. }
                | StructuralTypeShape::FixedArray { element, .. } => *element,
                _ => return None,
            };
            let backing = crate::storage::collection_backing::collection_shape_view_backing(
                types.clone(),
                shape,
                source_element,
                access,
            )?;
            let source = representation_element_layout(types.clone(), source_element)?;
            let target = representation_element_layout(types, element)?;
            if source.size == 0
                || target.size == 0
                || !source.size.is_multiple_of(target.size)
                || source.alignment < target.alignment
                || !source.alignment.is_multiple_of(target.alignment)
            {
                return None;
            }
            if !source.is_tightly_tiled() || !target.is_tightly_tiled() {
                return None;
            }
            Some(ElementViewRegionGeometry::WholeRepresentation {
                backing,
                source,
                target,
            })
        }
        ElementViewRegion::RepresentationBytes { .. } => {
            Some(ElementViewRegionGeometry::RepresentationBytes {
                byte_extent: representation_shape_byte_extent(types.clone(), shape)?,
                element: representation_element_layout(types, element)?,
            })
        }
    }
}
fn word() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid")
}
fn literal(value: u64) -> ScalarTerm {
    ScalarTerm::integer(word(), IntegerValue::Unsigned(u128::from(value))).expect("u64 literal")
}
fn value(value: ValueId) -> ScalarTerm {
    ScalarTerm::value(value, ScalarType::Integer(word()))
}
pub fn whole_representation_count_goal(
    source_length: ValueId,
    factor: u64,
) -> Option<Proposition> {
    if factor == 0 {
        return None;
    }
    Some(Proposition::LessOrEqual(
        value(source_length),
        literal(u64::MAX / factor),
    ))
}
/// The count goal a shared `WholeRepresentation` formation owns, from
/// the source collection's shape: `source_length * factor` fits `u64`,
/// where `factor` is how many target elements tile one source element.
pub fn whole_representation_region_goal<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    shape: &StructuralTypeShape,
    element: StructuralTypeId,
    region: ElementViewRegion,
    access: terminal_psi::StructuralAccess,
) -> Option<Proposition> {
    let ElementViewRegion::WholeRepresentation { source_length, .. } = region else {
        return None;
    };
    let Some(ElementViewRegionGeometry::WholeRepresentation { source, target, .. }) =
        element_view_shape_region_geometry(types, shape, element, region, access)
    else {
        return None;
    };
    whole_representation_count_goal(source_length, source.size.checked_div(target.size)?)
}
