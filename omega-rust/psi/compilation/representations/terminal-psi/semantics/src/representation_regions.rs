//! Portable, total byte representations for borrowed collection regions.
//!
//! Geometry is reconstructed from closed declarations, never a producer-supplied
//! stride or host ABI. Every admitted leaf accepts all stored bit patterns; bool,
//! bounded values, addresses and reference/descriptor storage require authority
//! this byte-region contract does not supply. The original loan still owns access
//! and lifetime. Ordinary exact-element collections do not use this layout.
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType, StructuralTypeId,
    ValueId,
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

/// Raw bytes grant only full bit-pattern validity. The typed layout owner is
/// shared, but restrictions and Boolean validity cannot be inferred from bytes.
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

/// Only fixed original octet storage supplies this region's extent. Capacity of
/// a variable-length owned byte sequence is not its initialized live extent.
pub fn representation_byte_extent<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    collection: StructuralTypeId,
) -> Option<u64> {
    let mut matches = types.clone().filter(|row| row.id == collection);
    let row = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
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
            let mut elements = types.filter(|row| row.id == *element);
            let element = elements.next()?;
            if elements.next().is_some() {
                return None;
            }
            matches!(element.shape, StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)) if !integer.is_address() && integer.sign() == IntegerSign::Unsigned && integer.bits()==8).then_some(*length)
        }
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
            capacity, ..
        }) => Some(*capacity),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub enum ElementViewRegionGeometry {
    WholeCollection(crate::CollectionBacking),
    RepresentationBytes {
        byte_extent: u64,
        element: RepresentationElementLayout,
    },
}
pub fn element_view_region_geometry<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    collection: StructuralTypeId,
    element: StructuralTypeId,
    region: ElementViewRegion,
    access: terminal_psi::StructuralAccess,
) -> Option<ElementViewRegionGeometry> {
    let mut declarations = types.clone().filter(|row| row.id == collection);
    let declaration = declarations.next()?;
    if declarations.next().is_some() {
        return None;
    }
    element_view_shape_region_geometry(types, &declaration.shape, element, region, access)
}

/// Geometry of an already selected collection occurrence. Inline byte fields
/// need no invented declaration identity; their carrier determines residence
/// and extent, while the element declaration and region retain their contracts.
pub fn element_view_shape_region_geometry<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    shape: &StructuralTypeShape,
    element: StructuralTypeId,
    region: ElementViewRegion,
    access: terminal_psi::StructuralAccess,
) -> Option<ElementViewRegionGeometry> {
    match region {
        ElementViewRegion::WholeCollection => {
            crate::collection_backing::collection_shape_view_backing(types, shape, element, access)
                .map(ElementViewRegionGeometry::WholeCollection)
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
/// Ordered proof legs own both extent and tiling; geometry alone creates none.
pub fn representation_region_goal(
    start: ValueId,
    end: ValueId,
    extent: u64,
    stride: u64,
) -> Option<Proposition> {
    if stride == 0 {
        return None;
    }
    let difference = ScalarTerm::exact_integer_subtract(word(), value(end), value(start)).ok()?;
    let remainder =
        ScalarTerm::exact_integer_remainder(word(), difference, literal(stride)).ok()?;
    Some(Proposition::Conjunction(vec![
        Proposition::LessOrEqual(value(start), value(end)),
        Proposition::LessOrEqual(value(end), literal(extent)),
        Proposition::Equal(remainder, literal(0)),
    ]))
}
pub fn representation_region_length(
    start: ValueId,
    end: ValueId,
    stride: u64,
) -> Option<ScalarTerm> {
    if stride == 0 {
        return None;
    }
    ScalarTerm::exact_integer_divide(
        word(),
        ScalarTerm::exact_integer_subtract(word(), value(end), value(start)).ok()?,
        literal(stride),
    )
    .ok()
}
