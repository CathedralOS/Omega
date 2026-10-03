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
    ByteSequenceCarrier, ElementViewRegion, StructuralFieldType, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape,
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

fn scalar_layout(scalar: ScalarType) -> Option<RepresentationElementLayout> {
    let size = match scalar {
        ScalarType::Integer(integer)
            if !integer.is_address() && matches!(integer.bits(), 8 | 16 | 32 | 64) =>
        {
            u64::from(integer.bits() / 8)
        }
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => 4,
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => 8,
        _ => return None,
    };
    Some(RepresentationElementLayout {
        size,
        alignment: size,
        leaves: vec![RepresentationScalarLeaf {
            path: Vec::new(),
            offset: 0,
            scalar_type: scalar,
        }],
    })
}
fn align(value: u64, alignment: u64) -> Option<u64> {
    value
        .checked_add(alignment.checked_sub(1)?)
        .map(|value| value / alignment * alignment)
}
/// Bounded declaration reconstruction; cycles, zero-stride and unsupported
/// validity/layout families remain unavailable rather than borrowing host size.
pub fn representation_element_layout<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    element: StructuralTypeId,
) -> Option<RepresentationElementLayout> {
    fn visit<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        element: StructuralTypeId,
        active: &mut Vec<StructuralTypeId>,
        budget: &mut usize,
    ) -> Option<RepresentationElementLayout> {
        *budget = budget.checked_sub(1)?;
        if active.len() >= 128 || active.contains(&element) {
            return None;
        }
        let mut matches = types.clone().filter(|row| row.id == element);
        let row = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        active.push(element);
        let layout = match &row.shape {
            StructuralTypeShape::PrimitiveScalar(scalar) => scalar_layout(*scalar)?,
            StructuralTypeShape::FixedArray { element, length } => {
                let child = visit(types.clone(), *element, active, budget)?;
                let stride = align(child.size, child.alignment)?;
                let count = usize::try_from(*length)
                    .ok()?
                    .checked_mul(child.leaves.len())?;
                *budget = budget.checked_sub(count)?;
                let mut leaves = Vec::new();
                leaves.try_reserve_exact(count).ok()?;
                for index in 0..*length {
                    for leaf in &child.leaves {
                        let mut path = vec![StructuralPathSegment::FixedIndex(index)];
                        path.extend_from_slice(&leaf.path);
                        leaves.push(RepresentationScalarLeaf {
                            path,
                            offset: stride.checked_mul(index)?.checked_add(leaf.offset)?,
                            scalar_type: leaf.scalar_type,
                        });
                    }
                }
                RepresentationElementLayout {
                    size: stride.checked_mul(*length)?,
                    alignment: child.alignment,
                    leaves,
                }
            }
            StructuralTypeShape::Record { fields } => {
                let mut offset = 0;
                let mut alignment = 1;
                let mut leaves = Vec::new();
                for field in fields {
                    if field.relevance.is_erased() {
                        return None;
                    }
                    let child = match &field.field_type {
                        StructuralFieldType::Scalar(scalar) => scalar_layout(*scalar)?,
                        StructuralFieldType::Structural(child) => {
                            visit(types.clone(), *child, active, budget)?
                        }
                        _ => return None,
                    };
                    *budget = budget.checked_sub(child.leaves.len())?;
                    offset = align(offset, child.alignment)?;
                    alignment = alignment.max(child.alignment);
                    leaves.try_reserve(child.leaves.len()).ok()?;
                    for leaf in child.leaves {
                        let mut path = vec![StructuralPathSegment::Field(field.identity.clone())];
                        path.extend(leaf.path);
                        leaves.push(RepresentationScalarLeaf {
                            path,
                            offset: offset.checked_add(leaf.offset)?,
                            scalar_type: leaf.scalar_type,
                        });
                    }
                    offset = offset.checked_add(child.size)?;
                }
                RepresentationElementLayout {
                    size: align(offset, alignment)?,
                    alignment,
                    leaves,
                }
            }
            _ => return None,
        };
        active.pop();
        (layout.size != 0 && !layout.leaves.is_empty()).then_some(layout)
    }
    visit(types, element, &mut Vec::new(), &mut 16384)
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
    match &row.shape {
        StructuralTypeShape::FixedArray { element, length } => {
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
) -> Option<ElementViewRegionGeometry> {
    match region {
        ElementViewRegion::WholeCollection => crate::collection_backing(types, collection, element)
            .map(ElementViewRegionGeometry::WholeCollection),
        ElementViewRegion::RepresentationBytes { .. } => {
            Some(ElementViewRegionGeometry::RepresentationBytes {
                byte_extent: representation_byte_extent(types.clone(), collection)?,
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

#[cfg(test)]
mod tests {
    use super::*;
    fn id(raw: u64) -> StructuralTypeId {
        StructuralTypeId::new(raw).unwrap()
    }
    fn scalar(bits: u16) -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap())
    }
    #[test]
    fn representation_region_layout_is_total_and_retains_nested_coordinates() {
        let mut types = vec![
            StructuralTypeDeclaration {
                id: id(1),
                identity: "word".into(),
                shape: StructuralTypeShape::PrimitiveScalar(scalar(16)),
            },
            StructuralTypeDeclaration {
                id: id(2),
                identity: "words".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: id(1),
                    length: 3,
                },
            },
        ];
        let layout = representation_element_layout(types.iter(), id(2)).unwrap();
        assert_eq!(layout.size, 6);
        assert_eq!(layout.leaves[2].offset, 4);
        assert_eq!(
            layout.leaves[2].path,
            vec![StructuralPathSegment::FixedIndex(2)]
        );
        types[0].shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean);
        assert!(representation_element_layout(types.iter(), id(2)).is_none());
        types[0].shape = StructuralTypeShape::FixedArray {
            element: id(2),
            length: 1,
        };
        assert!(representation_element_layout(types.iter(), id(2)).is_none());
    }
    #[test]
    fn representation_region_goal_retains_order_extent_and_exact_tiling() {
        let start = ValueId::new(1).unwrap();
        let end = ValueId::new(2).unwrap();
        let goal = representation_region_goal(start, end, 8, 2).unwrap();
        let Proposition::Conjunction(legs) = goal else {
            panic!()
        };
        assert_eq!(legs.len(), 3);
        assert_eq!(legs[0], Proposition::LessOrEqual(value(start), value(end)));
        assert_eq!(legs[1], Proposition::LessOrEqual(value(end), literal(8)));
        let Proposition::Conjunction(other_stride) =
            representation_region_goal(start, end, 8, 4).unwrap()
        else {
            panic!()
        };
        assert_ne!(legs[2], other_stride[2]);
        assert_ne!(
            representation_region_goal(end, start, 8, 2).unwrap(),
            Proposition::Conjunction(legs)
        );
        assert!(representation_region_goal(start, end, 8, 0).is_none());
        assert!(representation_region_length(start, end, 0).is_none());
    }
}
