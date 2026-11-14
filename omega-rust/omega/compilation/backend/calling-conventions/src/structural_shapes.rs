/*
ABI classification reads the native geometry owned by terminal_layout, then
classifies actual scalar leaves under the selected calling convention. It does
not reconstruct record sizes or substitute ordinary offsets for authored ones.
*/

use crate::terminal_layout::{
    NativeGeometry, StructuralLayoutResolver, checked_align_up_u32, scalar_shape,
};
use crate::{CallingPolicy, SystemVEightbyteClass, ValueClass, ValueShape};
use semantic_vocabulary::{ScalarType, StructuralTypeId};
use terminal_psi::{ByteSequenceCarrier, StructuralFieldType, StructuralTypeShape};

#[derive(Debug, PartialEq, Eq)]
pub struct InvalidStructuralShape;

pub fn classified_boundary_shape(
    structural_type: StructuralTypeId,
    geometry: &NativeGeometry<'_>,
    policy: CallingPolicy,
) -> Result<ValueShape, InvalidStructuralShape> {
    let layouts = &geometry.resolver;
    let declaration = layouts
        .declaration(structural_type)
        .map_err(|_| InvalidStructuralShape)?;
    let referent = layouts
        .shape(structural_type)
        .map_err(|_| InvalidStructuralShape)?;
    let class = match &declaration.shape {
        terminal_psi::record_or_fixed_array_shape_pattern!() => {
            let mut leaves = Vec::new();
            collect_scalar_leaves(structural_type, 0, layouts, &mut leaves, 0, None)?;
            classify_scalar_leaves(&leaves, referent, policy)?
        }
        StructuralTypeShape::PrimitiveScalar(_)
        | StructuralTypeShape::Reference { .. }
        | StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. })
        | StructuralTypeShape::ElementView { .. } => referent.class,
        _ => return Err(InvalidStructuralShape),
    };
    Ok(ValueShape { class, ..referent })
}

fn collect_scalar_leaves(
    structural_type: StructuralTypeId,
    base_offset: u32,
    layouts: &StructuralLayoutResolver<'_>,
    leaves: &mut Vec<(u32, u32, bool)>,
    depth: usize,
    authored_stride: Option<u32>,
) -> Result<(), InvalidStructuralShape> {
    if depth > 32 {
        return Err(InvalidStructuralShape);
    }
    let declaration = layouts
        .declaration(structural_type)
        .map_err(|_| InvalidStructuralShape)?;
    match &declaration.shape {
        StructuralTypeShape::PrimitiveScalar(scalar) => {
            leaves.push((
                base_offset,
                scalar_shape(*scalar).byte_size,
                matches!(scalar, ScalarType::IeeeFloat(_)),
            ));
        }
        StructuralTypeShape::Reference { .. }
        | StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. })
        | StructuralTypeShape::ElementView { .. } => {
            let referent = layouts
                .shape(structural_type)
                .map_err(|_| InvalidStructuralShape)?;
            leaves.push((base_offset, referent.byte_size, false));
        }
        StructuralTypeShape::Record { fields, .. } => {
            let mut local_offset = 0;
            for field in fields.iter().filter(|field| {
                !field.relevance.is_erased()
                    && !matches!(field.field_type, StructuralFieldType::Erased { .. })
            }) {
                let placement = layouts
                    .field_placement(field, structural_type, local_offset)
                    .map_err(|_| InvalidStructuralShape)?;
                let offset = base_offset
                    .checked_add(placement.byte_offset)
                    .ok_or(InvalidStructuralShape)?;
                if let Some(stored) = field
                    .storage
                    .as_ref()
                    .and_then(|storage| storage.stored_scalar())
                {
                    leaves.push((
                        offset,
                        placement.shape.byte_size,
                        matches!(stored, ScalarType::IeeeFloat(_)),
                    ));
                } else {
                    collect_field_leaves(
                        &field.field_type,
                        structural_type,
                        offset,
                        layouts,
                        leaves,
                        depth + 1,
                        placement.element_stride,
                    )?;
                }
                local_offset = placement
                    .byte_offset
                    .checked_add(placement.shape.byte_size)
                    .ok_or(InvalidStructuralShape)?;
            }
        }
        StructuralTypeShape::FixedArray {
            element,
            length: terminal_psi::ArrayExtent::Literal(length),
        } => {
            let element_shape = layouts
                .shape(*element)
                .map_err(|_| InvalidStructuralShape)?;
            let stride = authored_stride
                .or_else(|| {
                    checked_align_up_u32(
                        element_shape.byte_size,
                        u32::from(element_shape.alignment),
                    )
                })
                .ok_or(InvalidStructuralShape)?;
            for ordinal in 0..*length {
                let offset = u64::from(stride)
                    .checked_mul(ordinal)
                    .and_then(|offset| u64::from(base_offset).checked_add(offset))
                    .and_then(|offset| u32::try_from(offset).ok())
                    .ok_or(InvalidStructuralShape)?;
                collect_scalar_leaves(*element, offset, layouts, leaves, depth + 1, None)?;
            }
        }
        _ => return Err(InvalidStructuralShape),
    }
    Ok(())
}

fn collect_field_leaves(
    field: &StructuralFieldType,
    owner: StructuralTypeId,
    base_offset: u32,
    layouts: &StructuralLayoutResolver<'_>,
    leaves: &mut Vec<(u32, u32, bool)>,
    depth: usize,
    authored_stride: Option<u32>,
) -> Result<(), InvalidStructuralShape> {
    if let StructuralFieldType::Structural(nested) = field {
        return collect_scalar_leaves(
            *nested,
            base_offset,
            layouts,
            leaves,
            depth + 1,
            authored_stride,
        );
    }
    let leaf = layouts
        .field_shape(field, owner)
        .map_err(|_| InvalidStructuralShape)?;
    match field {
        StructuralFieldType::Scalar(ScalarType::IeeeFloat(_))
        | StructuralFieldType::IeeeFloat(_) => {
            leaves.push((base_offset, leaf.byte_size, true));
        }
        StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. })
        | StructuralFieldType::PredicateByteSequence {
            carrier: ByteSequenceCarrier::BoundedOwned { .. },
            ..
        } => {
            for offset in 0..leaf.byte_size {
                leaves.push((
                    base_offset
                        .checked_add(offset)
                        .ok_or(InvalidStructuralShape)?,
                    1,
                    false,
                ));
            }
        }
        _ => leaves.push((base_offset, leaf.byte_size, false)),
    }
    Ok(())
}

fn classify_scalar_leaves(
    leaves: &[(u32, u32, bool)],
    referent: ValueShape,
    policy: CallingPolicy,
) -> Result<ValueClass, InvalidStructuralShape> {
    if policy == CallingPolicy::MicrosoftX64 {
        return Ok(ValueClass::Integer);
    }
    if let Some(members) = homogeneous_float_leaves(leaves, referent) {
        if policy == CallingPolicy::Aapcs64
            || (policy == CallingPolicy::SystemVAMD64 && members > 1 && referent.byte_size <= 16)
        {
            return Ok(ValueClass::HomogeneousFloatAggregate { members });
        }
        if policy == CallingPolicy::SystemVAMD64 && members == 1 {
            return Ok(ValueClass::Float);
        }
    }
    if policy != CallingPolicy::SystemVAMD64 || referent.byte_size > 16 {
        return Ok(ValueClass::Integer);
    }
    let mut classes = [None, None];
    for &(offset, byte_size, is_float) in leaves {
        let eightbyte = usize::try_from(offset / 8).map_err(|_| InvalidStructuralShape)?;
        let last = offset
            .checked_add(byte_size)
            .and_then(|end| end.checked_sub(1))
            .ok_or(InvalidStructuralShape)?;
        if eightbyte > 1
            || usize::try_from(last / 8).map_err(|_| InvalidStructuralShape)? != eightbyte
        {
            return Err(InvalidStructuralShape);
        }
        classes[eightbyte] = Some(match classes[eightbyte] {
            Some(existing_is_sse) => existing_is_sse && is_float,
            None => is_float,
        });
    }
    let first = classes[0].ok_or(InvalidStructuralShape)?;
    let second = if referent.byte_size > 8 {
        classes[1].ok_or(InvalidStructuralShape)?
    } else {
        false
    };
    if !first && !second {
        return Ok(ValueClass::Integer);
    }
    if referent.byte_size <= 8 {
        return Err(InvalidStructuralShape);
    }
    Ok(ValueClass::SystemVAggregate {
        first: if first {
            SystemVEightbyteClass::Sse
        } else {
            SystemVEightbyteClass::Integer
        },
        second: if second {
            SystemVEightbyteClass::Sse
        } else {
            SystemVEightbyteClass::Integer
        },
    })
}

fn homogeneous_float_leaves(leaves: &[(u32, u32, bool)], referent: ValueShape) -> Option<u8> {
    let &(0, member_size, true) = leaves.first()? else {
        return None;
    };
    let members = u8::try_from(leaves.len()).ok()?;
    ((1..=4).contains(&members)
        && matches!(member_size, 4 | 8)
        && leaves
            .iter()
            .enumerate()
            .all(|(index, &(offset, size, is_float))| {
                is_float && size == member_size && offset == index as u32 * member_size
            })
        && referent.byte_size == member_size * u32::from(members)
        && referent.alignment == u16::try_from(member_size).unwrap_or(0))
    .then_some(members)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal_layout::{StructuralLayouts, TerminalLayoutError};
    use semantic_vocabulary::{
        AuthoredFieldStorage, AuthoredRecordStorage, IntegerSign, IntegerType, StructuralFieldId,
    };
    use terminal_psi::{
        ArrayExtent, BindingRelevance, LayoutApplicationId, StructuralAccess,
        StructuralFieldDeclaration, StructuralRecordStorage, StructuralTypeDeclaration,
        TerminalIdentityIndex,
    };

    fn declaration(id: u32, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
        StructuralTypeDeclaration {
            id: StructuralTypeId::new(u64::from(id)).unwrap(),
            identity: TerminalIdentityIndex(id),
            shape,
            requires: Vec::new(),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            copyable: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
        }
    }

    fn field(
        id: u32,
        scalar: ScalarType,
        offset: u64,
        stored: Option<ScalarType>,
    ) -> StructuralFieldDeclaration {
        StructuralFieldDeclaration {
            id: StructuralFieldId::new(u64::from(id)).unwrap(),
            identity: TerminalIdentityIndex(id),
            inline_carrier: None,
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Scalar(scalar),
            storage: Some(AuthoredFieldStorage::Scalar {
                byte_offset: offset,
                stored_scalar: stored,
            }),
        }
    }

    #[test]
    fn shared_geometry_is_demand_driven_and_matches_materialized_results() {
        let root = StructuralTypeId::new(1).unwrap();
        let deferred = StructuralTypeId::new(2).unwrap();
        let declarations = [
            declaration(1, StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean)),
            declaration(
                2,
                StructuralTypeShape::Record {
                    fields: Vec::new(),
                    storage: StructuralRecordStorage::TargetLayout(LayoutApplicationId(1)),
                },
            ),
        ];
        let query = StructuralLayoutResolver::new(declarations.iter()).unwrap();
        let table = StructuralLayouts::new(declarations.iter()).unwrap();
        assert_eq!(query.shape(root), Ok(ValueShape::integer(1, 1)));
        assert_eq!(query.shape(root), table.shape(root));
        let missing = StructuralTypeId::new(u64::MAX).unwrap();
        assert_eq!(
            query.shape(missing),
            Err(TerminalLayoutError::UnknownStructuralType(missing))
        );
        assert_eq!(
            query.shape(deferred),
            Err(TerminalLayoutError::UnclosedTargetLayout(deferred))
        );
    }

    #[test]
    fn shared_geometry_refuses_missing_recursive_and_invalid_storage() {
        let root = StructuralTypeId::new(1).unwrap();
        let missing = StructuralTypeId::new(2).unwrap();
        for shape in [
            StructuralTypeShape::Reference {
                referent: missing,
                access: StructuralAccess::SharedBorrow,
            },
            StructuralTypeShape::FixedArray {
                element: missing,
                length: ArrayExtent::Literal(0),
            },
            StructuralTypeShape::FixedArray {
                element: root,
                length: ArrayExtent::Literal(0),
            },
            StructuralTypeShape::Record {
                fields: Vec::new(),
                storage: StructuralRecordStorage::Authored(AuthoredRecordStorage {
                    byte_size: 8,
                    byte_alignment: 0,
                }),
            },
        ] {
            let declarations = [declaration(1, shape)];
            let query = StructuralLayoutResolver::new(declarations.iter()).unwrap();
            assert!(query.shape(root).is_err());
            assert!(
                query.shape(root).is_err(),
                "failed queries must not leave an accepted cached shape"
            );
        }
        assert_eq!(checked_align_up_u32(4, 0), None);
        let mut high = declaration(1, StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean));
        high.id = StructuralTypeId::new(u64::MAX).unwrap();
        let query = StructuralLayoutResolver::new([&high]).unwrap();
        assert_eq!(query.shape(high.id), Ok(ValueShape::integer(1, 1)));
    }

    #[test]
    fn boundary_geometry_uses_authored_offsets_and_stored_widths() {
        let root = StructuralTypeId::new(1).unwrap();
        let integer =
            |bits| ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap());
        let declarations = [declaration(
            1,
            StructuralTypeShape::Record {
                storage: StructuralRecordStorage::Authored(AuthoredRecordStorage {
                    byte_size: 16,
                    byte_alignment: 8,
                }),
                fields: vec![
                    field(
                        1,
                        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64),
                        8,
                        None,
                    ),
                    field(2, integer(64), 0, Some(integer(32))),
                ],
            },
        )];
        let query = StructuralLayoutResolver::new(declarations.iter()).unwrap();
        let mut leaves = Vec::new();
        collect_scalar_leaves(root, 0, &query, &mut leaves, 0, None).unwrap();
        assert_eq!(leaves, [(8, 8, true), (0, 4, false)]);
        assert_eq!(
            classified_boundary_shape(
                root,
                &NativeGeometry::new(&declarations).unwrap(),
                CallingPolicy::SystemVAMD64
            )
            .unwrap()
            .class,
            ValueClass::SystemVAggregate {
                first: SystemVEightbyteClass::Integer,
                second: SystemVEightbyteClass::Sse
            }
        );
    }

    #[test]
    fn boundary_geometry_repeats_aligned_element_footprints() {
        let root = StructuralTypeId::new(1).unwrap();
        let element = StructuralTypeId::new(2).unwrap();
        let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let declarations = [
            declaration(
                1,
                StructuralTypeShape::FixedArray {
                    element,
                    length: ArrayExtent::Literal(2),
                },
            ),
            declaration(
                2,
                StructuralTypeShape::Record {
                    storage: StructuralRecordStorage::Authored(AuthoredRecordStorage {
                        byte_size: 12,
                        byte_alignment: 8,
                    }),
                    fields: vec![field(1, scalar, 0, None)],
                },
            ),
        ];
        let query = StructuralLayoutResolver::new(declarations.iter()).unwrap();
        let mut leaves = Vec::new();
        collect_scalar_leaves(root, 0, &query, &mut leaves, 0, None).unwrap();
        assert_eq!(query.shape(root), Ok(ValueShape::integer(32, 8)));
        assert_eq!(leaves, [(0, 8, false), (16, 8, false)]);
    }

    #[test]
    fn boundary_geometry_refuses_noncontiguous_scalar_placement() {
        let root = StructuralTypeId::new(1).unwrap();
        let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap());
        let mut fragmented = field(1, scalar, 0, None);
        fragmented.storage = Some(AuthoredFieldStorage::Fragmented(
            semantic_vocabulary::AuthoredBitFragments::new([
                semantic_vocabulary::AuthoredBitFragment {
                    container_byte_offset: 0,
                    container_width_bits: 8,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 8,
                },
                semantic_vocabulary::AuthoredBitFragment {
                    container_byte_offset: 2,
                    container_width_bits: 8,
                    destination_lsb: 0,
                    source_lsb: 8,
                    width: 8,
                },
            ]),
        ));
        let declarations = [declaration(
            1,
            StructuralTypeShape::Record {
                storage: StructuralRecordStorage::Authored(AuthoredRecordStorage {
                    byte_size: 4,
                    byte_alignment: 1,
                }),
                fields: vec![fragmented],
            },
        )];
        assert!(
            classified_boundary_shape(
                root,
                &NativeGeometry::new(&declarations).unwrap(),
                CallingPolicy::SystemVAMD64
            )
            .is_err()
        );
    }
}
