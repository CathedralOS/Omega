//! Physical projection replay from canonical finite declarations.

use std::collections::BTreeMap;

use calling_conventions::ValueShape;
use semantic_vocabulary::{IeeeFloatFormat, ScalarType, StructuralTypeId};
use terminal_psi::{
    ByteSequenceCarrier, StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

pub(super) struct Layouts<'a> {
    declarations: BTreeMap<StructuralTypeId, &'a StructuralTypeDeclaration>,
    shapes: BTreeMap<StructuralTypeId, ValueShape>,
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
        })
    }

    pub(super) fn shape(&mut self, structural_type: StructuralTypeId) -> Option<ValueShape> {
        if let Some(shape) = self.shapes.get(&structural_type) {
            return Some(*shape);
        }
        let declaration = *self.declarations.get(&structural_type)?;
        let shape = match &declaration.shape {
            StructuralTypeShape::Record { fields } => {
                let mut size = 0_u32;
                let mut alignment = 1;
                for field in fields {
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
            StructuralTypeShape::FixedArray { element, length } => {
                let shape = self.shape(*element)?;
                let stride = align(u32::from(shape.byte_size), shape.alignment)?;
                let size = u64::from(stride).checked_mul(*length)?;
                ValueShape::integer(u16::try_from(size).ok()?, shape.alignment)
            }
            _ => return None,
        };
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
