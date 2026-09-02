use super::shared::*;

pub(crate) fn structural_shape(
    structural_type: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<ValueShape, LoweringError> {
    if let Some(shape) = cache.get(&structural_type) {
        return Ok(*shape);
    }
    if !active.insert(structural_type) {
        return Err(LoweringError::RecursiveStructuralType(structural_type));
    }
    let result = (|| {
        let declaration = declarations
            .get(&structural_type)
            .copied()
            .ok_or(LoweringError::UnknownStructuralType(structural_type))?;
        match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(_) => Err(
                LoweringError::UnsupportedStructuralPrimitiveScalar(structural_type),
            ),
            StructuralTypeShape::ByteSequence(_) => Err(
                LoweringError::UnsupportedStructuralByteSequence(structural_type),
            ),
            StructuralTypeShape::Record { fields } => {
                if fields.is_empty() {
                    return Ok(ValueShape::integer(0, 1));
                }
                let mut byte_size = 0_u32;
                let mut alignment = 1_u16;
                for field in fields {
                    if field.relevance.is_erased() {
                        continue;
                    }
                    let field_shape = match &field.field_type {
                        StructuralFieldType::Scalar(ScalarType::Boolean) => {
                            ValueShape::integer(1, 1)
                        }
                        StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
                            let size = integer.bits().div_ceil(8);
                            let field_alignment = size.next_power_of_two().min(16);
                            ValueShape::integer(size, field_alignment)
                        }
                        StructuralFieldType::Scalar(ScalarType::IeeeFloat(
                            IeeeFloatFormat::Binary32,
                        )) => ValueShape::float(4),
                        StructuralFieldType::Scalar(ScalarType::IeeeFloat(
                            IeeeFloatFormat::Binary64,
                        )) => ValueShape::float(8),
                        StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32) => {
                            ValueShape::float(4)
                        }
                        StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary64) => {
                            ValueShape::float(8)
                        }
                        StructuralFieldType::ByteSequence(carrier) => {
                            byte_sequence_shape(*carrier, structural_type)?
                        }
                        StructuralFieldType::Structural(nested) => {
                            structural_shape(*nested, declarations, cache, active)?
                        }
                        // Erased capability/proof fields remain semantically
                        // relevant but deliberately contribute no target
                        // bytes. A later attempt to project such a field still
                        // fails because it has no structural runtime shape.
                        StructuralFieldType::Erased { .. } => continue,
                    };
                    alignment = alignment.max(field_shape.alignment);
                    byte_size = checked_align_up_u32(byte_size, u32::from(field_shape.alignment))
                        .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
                    byte_size = byte_size
                        .checked_add(u32::from(field_shape.byte_size))
                        .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
                }
                byte_size = checked_align_up_u32(byte_size, u32::from(alignment))
                    .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
                if byte_size == 0 {
                    return Err(LoweringError::EmptyStructuralType(structural_type));
                }
                let byte_size = u16::try_from(byte_size)
                    .map_err(|_| LoweringError::StructuralTypeTooLarge(structural_type))?;
                Ok(ValueShape::integer(byte_size, alignment))
            }
            StructuralTypeShape::FixedArray { element, length } => {
                if *length == 0 {
                    return Err(LoweringError::EmptyStructuralType(structural_type));
                }
                let element = structural_shape(*element, declarations, cache, active)?;
                let stride = checked_align_up_u32(
                    u32::from(element.byte_size),
                    u32::from(element.alignment),
                )
                .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
                let byte_size = u64::from(stride)
                    .checked_mul(*length)
                    .and_then(|size| u16::try_from(size).ok())
                    .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
                Ok(ValueShape::integer(byte_size, element.alignment))
            }
            StructuralTypeShape::Sum { .. } | StructuralTypeShape::Mixed { .. } => {
                Err(LoweringError::UnsupportedStructuralSum(structural_type))
            }
        }
    })();
    active.remove(&structural_type);
    let shape = result?;
    cache.insert(structural_type, shape);
    Ok(shape)
}

pub(super) fn direct_boolean_field_offset(
    structural_type: StructuralTypeId,
    field: StructuralFieldId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<u32, LoweringError> {
    direct_scalar_field_offset(structural_type, field, ScalarType::Boolean, declarations)
}

pub(super) fn direct_integer_field_offset(
    structural_type: StructuralTypeId,
    field: StructuralFieldId,
    scalar_type: psi_core::IntegerType,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<u32, LoweringError> {
    direct_scalar_field_offset(
        structural_type,
        field,
        ScalarType::Integer(scalar_type),
        declarations,
    )
}

fn direct_scalar_field_offset(
    structural_type: StructuralTypeId,
    field: StructuralFieldId,
    expected_type: ScalarType,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<u32, LoweringError> {
    let declaration = declarations
        .get(&structural_type)
        .copied()
        .ok_or(LoweringError::UnknownStructuralType(structural_type))?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return Err(LoweringError::UnknownStructuralType(structural_type));
    };
    let mut cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let mut offset = 0_u32;
    for candidate in fields
        .iter()
        .filter(|candidate| !candidate.relevance.is_erased())
    {
        let shape = match candidate.field_type {
            StructuralFieldType::Scalar(ScalarType::Boolean) => ValueShape::integer(1, 1),
            StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
                let size = integer.bits().div_ceil(8);
                ValueShape::integer(size, size.next_power_of_two().min(16))
            }
            StructuralFieldType::Scalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32)) => {
                ValueShape::float(4)
            }
            StructuralFieldType::Scalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)) => {
                ValueShape::float(8)
            }
            StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32) => ValueShape::float(4),
            StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary64) => ValueShape::float(8),
            StructuralFieldType::ByteSequence(carrier) => {
                byte_sequence_shape(carrier, structural_type)?
            }
            StructuralFieldType::Structural(nested) => {
                structural_shape(nested, declarations, &mut cache, &mut active)?
            }
            StructuralFieldType::Erased { .. } => continue,
        };
        offset = checked_align_up_u32(offset, u32::from(shape.alignment))
            .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
        if candidate.id == field {
            return (candidate.field_type == StructuralFieldType::Scalar(expected_type))
                .then_some(offset)
                .ok_or(LoweringError::UnknownStructuralType(structural_type));
        }
        offset = offset
            .checked_add(u32::from(shape.byte_size))
            .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
    }
    Err(LoweringError::UnknownStructuralType(structural_type))
}

pub(super) fn resolve_structural_field_path(
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<(StructuralTypeId, ValueShape, u32), LoweringError> {
    let root_type = structural_type;
    let mut total_offset = 0_u32;
    let mut selected_shape = None;
    for segment in path {
        let StructuralPathSegment::Field(identity) = segment else {
            return Err(LoweringError::UnknownStructuralType(structural_type));
        };
        let declaration = declarations
            .get(&structural_type)
            .copied()
            .ok_or(LoweringError::UnknownStructuralType(structural_type))?;
        let StructuralTypeShape::Record { fields } = &declaration.shape else {
            return Err(LoweringError::UnknownStructuralType(structural_type));
        };
        let mut local_offset = 0_u32;
        let mut selected = None;
        for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
            let field_shape = match field.field_type {
                StructuralFieldType::Scalar(ScalarType::Boolean) => ValueShape::integer(1, 1),
                StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
                    let size = integer.bits().div_ceil(8);
                    ValueShape::integer(size, size.next_power_of_two().min(16))
                }
                StructuralFieldType::Scalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32)) => {
                    ValueShape::float(4)
                }
                StructuralFieldType::Scalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)) => {
                    ValueShape::float(8)
                }
                StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32) => ValueShape::float(4),
                StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary64) => ValueShape::float(8),
                StructuralFieldType::ByteSequence(carrier) => {
                    byte_sequence_shape(carrier, structural_type)?
                }
                StructuralFieldType::Structural(nested) => {
                    structural_shape(nested, declarations, cache, active)?
                }
                StructuralFieldType::Erased { .. } => continue,
            };
            local_offset = checked_align_up_u32(local_offset, u32::from(field_shape.alignment))
                .ok_or(LoweringError::StructuralTypeTooLarge(root_type))?;
            if field.identity == *identity {
                let StructuralFieldType::Structural(field_type) = field.field_type else {
                    return Err(LoweringError::UnknownStructuralType(structural_type));
                };
                selected = Some((field_type, field_shape, local_offset));
                break;
            }
            local_offset = local_offset
                .checked_add(u32::from(field_shape.byte_size))
                .ok_or(LoweringError::StructuralTypeTooLarge(root_type))?;
        }
        let (field_type, field_shape, field_offset) =
            selected.ok_or(LoweringError::UnknownStructuralType(structural_type))?;
        total_offset = total_offset
            .checked_add(field_offset)
            .ok_or(LoweringError::StructuralTypeTooLarge(root_type))?;
        structural_type = field_type;
        selected_shape = Some(field_shape);
    }
    selected_shape
        .map(|shape| (structural_type, shape, total_offset))
        .ok_or(LoweringError::UnknownStructuralType(root_type))
}

pub(super) fn byte_sequence_shape(
    carrier: psi_terminal::ByteSequenceCarrier,
    structural_type: StructuralTypeId,
) -> Result<ValueShape, LoweringError> {
    let byte_size = match carrier {
        // Current native targets are 64-bit. The semantic carrier
        // deliberately does not retain the physical descriptor fields.
        psi_terminal::ByteSequenceCarrier::BorrowedView => 16_u64,
        psi_terminal::ByteSequenceCarrier::BoundedOwned { capacity } => capacity
            .checked_add(8)
            .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?,
    };
    Ok(ValueShape::integer(
        u16::try_from(byte_size)
            .map_err(|_| LoweringError::StructuralTypeTooLarge(structural_type))?,
        8,
    ))
}

pub(super) fn expected_maximal_residual_subtrees(
    root_type: StructuralTypeId,
    moved: &[(Vec<StructuralPathSegment>, StructuralTypeId)],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Option<Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>> {
    if moved.is_empty() {
        return None;
    }
    if moved
        .iter()
        .all(|(path, _)| matches!(path.as_slice(), [StructuralPathSegment::FixedIndex(_)]))
    {
        let declaration = declarations.get(&root_type).copied()?;
        let StructuralTypeShape::FixedArray { element, length } = declaration.shape else {
            return None;
        };
        if !matches!((length, moved.len()), (2, 1) | (3, 1 | 2) | (4, 2))
            || moved.iter().any(|(_, moved_type)| *moved_type != element)
            || !matches!(
                declarations
                    .get(&element)
                    .map(|declaration| &declaration.shape),
                Some(StructuralTypeShape::Record { .. })
            )
        {
            return None;
        }
        let moved_indexes = moved
            .iter()
            .filter_map(|(path, _)| match path.as_slice() {
                [StructuralPathSegment::FixedIndex(index)] if *index < length => Some(*index),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        if moved_indexes.len() != moved.len() {
            return None;
        }
        let residuals = (0..length)
            .rev()
            .filter(|index| !moved_indexes.contains(index))
            .map(|index| (vec![StructuralPathSegment::FixedIndex(index)], element))
            .collect::<Vec<_>>();
        return (!residuals.is_empty()).then_some(residuals);
    }
    if moved.iter().all(|(path, _)| {
        matches!(
            path.as_slice(),
            [
                StructuralPathSegment::FixedIndex(_),
                StructuralPathSegment::FixedIndex(_)
            ]
        )
    }) {
        let StructuralTypeShape::FixedArray { element, length: 2 } =
            declarations.get(&root_type)?.shape
        else {
            return None;
        };
        let StructuralTypeShape::FixedArray {
            element: leaf,
            length: inner_length @ (3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11),
        } = declarations.get(&element)?.shape
        else {
            return None;
        };
        if moved.len() != 2
            || moved.iter().any(|(_, moved_type)| *moved_type != leaf)
            || !matches!(
                declarations
                    .get(&leaf)
                    .map(|declaration| &declaration.shape),
                Some(StructuralTypeShape::Record { .. })
            )
        {
            return None;
        }
        let moved_by_outer = moved
            .iter()
            .filter_map(|(path, _)| match path.as_slice() {
                [
                    StructuralPathSegment::FixedIndex(outer @ (0 | 1)),
                    StructuralPathSegment::FixedIndex(inner),
                ] if *inner < inner_length => Some((*outer, *inner)),
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();
        if moved_by_outer.len() != 2 {
            return None;
        }
        let mut residuals = Vec::with_capacity(usize::try_from(2 * (inner_length - 1)).ok()?);
        for outer in (0_u64..2).rev() {
            let moved_inner = *moved_by_outer.get(&outer)?;
            for inner in (0_u64..inner_length).rev() {
                if inner != moved_inner {
                    residuals.push((
                        vec![
                            StructuralPathSegment::FixedIndex(outer),
                            StructuralPathSegment::FixedIndex(inner),
                        ],
                        leaf,
                    ));
                }
            }
        }
        return Some(residuals);
    }
    let borrowed = moved
        .iter()
        .map(|(path, structural_type)| (path.as_slice(), *structural_type))
        .collect::<Vec<_>>();
    let mut residuals = Vec::new();
    append_maximal_residual_subtrees(root_type, &[], &borrowed, declarations, &mut residuals)?;
    (!residuals.is_empty()).then_some(residuals)
}

pub(super) fn is_partial_cleanup_path(path: &[StructuralPathSegment]) -> bool {
    (!path.is_empty()
        && path.iter().all(
            |segment| matches!(segment, StructuralPathSegment::Field(identity) if !identity.is_empty()),
        )) || matches!(
        path,
        [StructuralPathSegment::FixedIndex(0 | 1 | 2 | 3)]
            | [
                StructuralPathSegment::FixedIndex(0 | 1),
                StructuralPathSegment::FixedIndex(0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10),
            ]
    )
}

pub(super) fn append_maximal_residual_subtrees(
    structural_type: StructuralTypeId,
    prefix: &[StructuralPathSegment],
    moved: &[(&[StructuralPathSegment], StructuralTypeId)],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
) -> Option<()> {
    let declaration = declarations.get(&structural_type).copied()?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return None;
    };
    if fields.is_empty()
        || fields.iter().any(|field| {
            field.relevance.is_erased()
                || !matches!(
                    field.field_type,
                    StructuralFieldType::Structural(_)
                        | StructuralFieldType::Scalar(_)
                        | StructuralFieldType::IeeeFloat(_)
                        | StructuralFieldType::ByteSequence(
                            psi_terminal::ByteSequenceCarrier::BoundedOwned { .. }
                        )
                )
        })
        || moved
            .iter()
            .any(|(path, _)| !matches!(path.first(), Some(StructuralPathSegment::Field(_))))
    {
        return None;
    }
    let mut matched = 0_usize;
    for field in fields.iter().rev() {
        let matching = moved
            .iter()
            .filter(|(path, _)| {
                matches!(path.first(), Some(StructuralPathSegment::Field(identity))
                    if identity == &field.identity)
            })
            .copied()
            .collect::<Vec<_>>();
        matched += matching.len();
        let mut field_path = prefix.to_vec();
        field_path.push(StructuralPathSegment::Field(field.identity.clone()));
        let StructuralFieldType::Structural(field_type) = field.field_type else {
            if !matching.is_empty() {
                return None;
            }
            continue;
        };
        if matching.is_empty() {
            residuals.push((field_path, field_type));
            continue;
        }
        let whole_moves = matching
            .iter()
            .filter(|(path, _)| path.len() == 1)
            .collect::<Vec<_>>();
        if !whole_moves.is_empty() {
            if whole_moves.len() != 1 || matching.len() != 1 || whole_moves[0].1 != field_type {
                return None;
            }
            continue;
        }
        let nested = matching
            .iter()
            .map(|(path, leaf_type)| (&path[1..], *leaf_type))
            .collect::<Vec<_>>();
        append_maximal_residual_subtrees(
            field_type,
            &field_path,
            &nested,
            declarations,
            residuals,
        )?;
    }
    (matched == moved.len()).then_some(())
}

pub(super) fn checked_align_up_u32(value: u32, alignment: u32) -> Option<u32> {
    let remainder = value % alignment;
    if remainder == 0 {
        Some(value)
    } else {
        value.checked_add(alignment - remainder)
    }
}
