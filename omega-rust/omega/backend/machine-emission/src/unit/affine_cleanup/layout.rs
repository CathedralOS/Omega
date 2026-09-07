//! Physical projection replay from canonical finite declarations.

use std::collections::{BTreeMap, BTreeSet};

use calling_conventions::ValueShape;
use semantic_vocabulary::{IeeeFloatFormat, ScalarType, StructuralTypeId};
use terminal_psi::{
    ByteSequenceCarrier, StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

pub(super) struct Layouts<'a> {
    declarations: BTreeMap<StructuralTypeId, &'a StructuralTypeDeclaration>,
    shapes: BTreeMap<StructuralTypeId, ValueShape>,
    active: BTreeSet<StructuralTypeId>,
}

impl<'a> Layouts<'a> {
    pub(super) fn new(
        declarations: &'a [StructuralTypeDeclaration],
        root: StructuralTypeId,
    ) -> Option<Self> {
        Some(Self {
            declarations: crate::cleanup::partial_partition::canonical_finite_declarations(
                declarations,
                root,
            )?,
            shapes: BTreeMap::new(),
            active: BTreeSet::new(),
        })
    }

    /// Material shape replay does not grant projected-cleanup admission.
    /// Keep `new` behind the existing finite ownership closure check.
    pub(super) fn for_material(declarations: &'a [StructuralTypeDeclaration]) -> Option<Self> {
        let by_id = declarations
            .iter()
            .map(|declaration| (declaration.id, declaration))
            .collect::<BTreeMap<_, _>>();
        (by_id.len() == declarations.len()).then_some(Self {
            declarations: by_id,
            shapes: BTreeMap::new(),
            active: BTreeSet::new(),
        })
    }

    pub(super) fn shape(&mut self, structural_type: StructuralTypeId) -> Option<ValueShape> {
        if let Some(shape) = self.shapes.get(&structural_type) {
            return Some(*shape);
        }
        if !self.active.insert(structural_type) {
            return None;
        }
        let declaration = *self.declarations.get(&structural_type)?;
        let shape = match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)) => {
                let size = integer.bits().div_ceil(8);
                ValueShape::integer(size, size.checked_next_power_of_two()?.min(8))
            }
            StructuralTypeShape::PrimitiveScalar(scalar) => {
                self.field_shape(&StructuralFieldType::Scalar(*scalar))?
            }
            StructuralTypeShape::Record { fields } => {
                let mut size = 0_u32;
                let mut alignment = 1;
                for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                    let shape = self.field_shape(&field.field_type)?;
                    alignment = alignment.max(shape.alignment);
                    size = align(size, shape.alignment)?.checked_add(u32::from(shape.byte_size))?;
                }
                size = align(size, alignment)?;
                if !fields.is_empty() && size == 0 {
                    return None;
                }
                ValueShape::integer(u16::try_from(size).ok()?, alignment)
            }
            StructuralTypeShape::FixedArray { element, length } if *length != 0 => {
                let shape = self.shape(*element)?;
                let stride = align(u32::from(shape.byte_size), shape.alignment)?;
                let size = u64::from(stride).checked_mul(*length)?;
                ValueShape::integer(u16::try_from(size).ok()?, shape.alignment)
            }
            _ => return None,
        };
        self.active.remove(&structural_type);
        self.shapes.insert(structural_type, shape);
        Some(shape)
    }

    fn field_shape(&mut self, field: &StructuralFieldType) -> Option<ValueShape> {
        match *field {
            StructuralFieldType::Structural(nested) => self.shape(nested),
            StructuralFieldType::Scalar(ScalarType::Boolean) => Some(ValueShape::integer(1, 1)),
            StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
                let size = integer.bits().div_ceil(8);
                Some(ValueShape::integer(
                    size,
                    size.checked_next_power_of_two()?.min(16),
                ))
            }
            StructuralFieldType::Scalar(ScalarType::IeeeFloat(format))
            | StructuralFieldType::IeeeFloat(format) => Some(ValueShape::float(match format {
                IeeeFloatFormat::Binary32 => 4,
                IeeeFloatFormat::Binary64 => 8,
            })),
            StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity }) => {
                Some(ValueShape::integer(
                    u16::try_from(capacity.checked_add(8)?).ok()?,
                    8,
                ))
            }
            _ => None,
        }
    }

    pub(super) fn root_array_metadata(
        &mut self,
        root: StructuralTypeId,
    ) -> Option<(Option<u64>, Option<u32>)> {
        match self.declarations.get(&root)?.shape {
            StructuralTypeShape::FixedArray { element, length } => {
                let element = self.shape(element)?;
                Some((
                    Some(length),
                    Some(align(u32::from(element.byte_size), element.alignment)?),
                ))
            }
            StructuralTypeShape::Record { .. } => Some((None, None)),
            _ => None,
        }
    }

    pub(super) fn project(
        &mut self,
        mut structural_type: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<(StructuralTypeId, ValueShape, u32)> {
        let mut offset = 0_u32;
        for segment in path {
            let declaration = *self.declarations.get(&structural_type)?;
            let (nested, local_offset) = match (&declaration.shape, segment) {
                (
                    StructuralTypeShape::Record { fields },
                    StructuralPathSegment::Field(identity),
                ) => {
                    let mut local_offset = 0_u32;
                    let mut selected = None;
                    for field in fields {
                        let shape = self.field_shape(&field.field_type)?;
                        local_offset = align(local_offset, shape.alignment)?;
                        if field.identity == *identity {
                            let StructuralFieldType::Structural(nested) = field.field_type else {
                                return None;
                            };
                            selected = Some((nested, local_offset));
                            break;
                        }
                        local_offset = local_offset.checked_add(u32::from(shape.byte_size))?;
                    }
                    selected?
                }
                (
                    StructuralTypeShape::FixedArray { element, length },
                    StructuralPathSegment::FixedIndex(index),
                ) if index < length => {
                    let shape = self.shape(*element)?;
                    let stride = align(u32::from(shape.byte_size), shape.alignment)?;
                    let local_offset = u64::from(stride).checked_mul(*index)?;
                    (*element, u32::try_from(local_offset).ok()?)
                }
                _ => return None,
            };
            offset = offset.checked_add(local_offset)?;
            structural_type = nested;
        }
        Some((structural_type, self.shape(structural_type)?, offset))
    }
}

fn align(size: u32, alignment: u16) -> Option<u32> {
    size.checked_next_multiple_of(u32::from(alignment))
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerSign, IntegerType};

    fn declaration(identifier: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
        StructuralTypeDeclaration {
            id: StructuralTypeId::new(identifier).unwrap(),
            identity: format!("Type{identifier}"),
            shape,
        }
    }

    fn primitive_array(length: u64) -> [StructuralTypeDeclaration; 2] {
        [
            declaration(
                1,
                StructuralTypeShape::FixedArray {
                    element: StructuralTypeId::new(2).unwrap(),
                    length,
                },
            ),
            declaration(
                2,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                )),
            ),
        ]
    }

    #[test]
    fn material_primitive_arrays_do_not_widen_cleanup_admission() {
        let declarations = primitive_array(3);
        let root = declarations[0].id;
        let mut layouts = Layouts::for_material(&declarations).unwrap();
        assert_eq!(layouts.shape(root), Some(ValueShape::integer(24, 8)));
        assert_eq!(layouts.shape(root), Some(ValueShape::integer(24, 8)));
        assert!(Layouts::new(&declarations, root).is_none());
    }

    #[test]
    fn material_replay_rejects_cycles_missing_leaves_empty_arrays_and_overflow() {
        let declarations = primitive_array(3);
        let root = declarations[0].id;
        let mut cyclic = declarations.clone();
        cyclic[1].shape = StructuralTypeShape::FixedArray {
            element: root,
            length: 1,
        };
        let mut duplicate = declarations.to_vec();
        duplicate.push(declarations[1].clone());
        assert!(Layouts::for_material(&duplicate).is_none());
        for invalid in [
            cyclic.to_vec(),
            declarations[..1].to_vec(),
            primitive_array(0).to_vec(),
            primitive_array(u64::MAX).to_vec(),
        ] {
            assert_eq!(Layouts::for_material(&invalid).unwrap().shape(root), None);
        }
    }
}
