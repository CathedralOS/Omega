mod residual_cleanup;

use super::shared::*;

pub(super) use residual_cleanup::{expected_maximal_residual_subtrees, is_partial_cleanup_path};

/// Existing copy metadata describes the root array, not the final index in a
/// nested path. Record-root projections have no root array metadata.
pub(super) fn root_array_projection_metadata(
    root: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<(Option<u64>, Option<u32>), LoweringError> {
    let declaration = declarations
        .get(&root)
        .ok_or(LoweringError::UnknownStructuralType(root))?;
    let StructuralTypeShape::FixedArray { element, length } = declaration.shape else {
        return Ok((None, None));
    };
    let shape = structural_shape(element, declarations, cache, active)?;
    let stride = checked_align_up_u32(u32::from(shape.byte_size), u32::from(shape.alignment))
        .ok_or(LoweringError::StructuralTypeTooLarge(root))?;
    Ok((Some(length), Some(stride)))
}

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
            StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean) => {
                Ok(ValueShape::integer(1, 1))
            }
            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)) => {
                let byte_size = integer.bits().div_ceil(8);
                Ok(ValueShape::integer(
                    byte_size,
                    byte_size.next_power_of_two().min(8),
                ))
            }
            StructuralTypeShape::PrimitiveScalar(ScalarType::IeeeFloat(
                IeeeFloatFormat::Binary32,
            )) => Ok(ValueShape::float(4)),
            StructuralTypeShape::PrimitiveScalar(ScalarType::IeeeFloat(
                IeeeFloatFormat::Binary64,
            )) => Ok(ValueShape::float(8)),
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
                    let field_shape = structural_field_shape(
                        &field.field_type,
                        structural_type,
                        declarations,
                        cache,
                        active,
                    )?;
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
            StructuralTypeShape::Sum { cases } => conventional_sum_layout_from_parts(
                structural_type,
                &[],
                cases,
                declarations,
                cache,
                active,
            )
            .map(|layout| layout.shape),
            StructuralTypeShape::Mixed { fields, cases } => conventional_sum_layout_from_parts(
                structural_type,
                fields,
                cases,
                declarations,
                cache,
                active,
            )
            .map(|layout| layout.shape),
        }
    })();
    active.remove(&structural_type);
    let shape = result?;
    cache.insert(structural_type, shape);
    Ok(shape)
}

pub(super) fn structural_parameter_shape(
    referent: ValueShape,
    access: StructuralAccess,
) -> ValueShape {
    if matches!(
        access,
        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
    ) {
        ValueShape::borrowed_reference(referent.byte_size, referent.alignment)
    } else {
        referent
    }
}

pub(super) fn structural_sum_layout(
    structural_type: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<calling_conventions::ConventionalSumLayout, LoweringError> {
    let declaration = declarations
        .get(&structural_type)
        .copied()
        .ok_or(LoweringError::UnknownStructuralType(structural_type))?;
    match &declaration.shape {
        StructuralTypeShape::Sum { cases } => conventional_sum_layout_from_parts(
            structural_type,
            &[],
            cases,
            declarations,
            cache,
            active,
        ),
        StructuralTypeShape::Mixed { fields, cases } => conventional_sum_layout_from_parts(
            structural_type,
            fields,
            cases,
            declarations,
            cache,
            active,
        ),
        _ => Err(LoweringError::UnsupportedStructuralSum(structural_type)),
    }
}

fn conventional_sum_layout_from_parts(
    structural_type: StructuralTypeId,
    common_fields: &[terminal_psi::StructuralFieldDeclaration],
    cases: &[terminal_psi::StructuralCaseDeclaration],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<calling_conventions::ConventionalSumLayout, LoweringError> {
    if cases.is_empty() {
        return Err(LoweringError::EmptyStructuralType(structural_type));
    }
    let common = common_fields
        .iter()
        .filter(|field| !field.relevance.is_erased())
        .map(|field| {
            structural_field_shape(
                &field.field_type,
                structural_type,
                declarations,
                cache,
                active,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let payloads = cases
        .iter()
        .map(|case| {
            case.fields
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .map(|field| {
                    structural_field_shape(
                        &field.field_type,
                        structural_type,
                        declarations,
                        cache,
                        active,
                    )
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    calling_conventions::evaluate_conventional_sum_layout(&common, &payloads)
        .map_err(|_| LoweringError::StructuralTypeTooLarge(structural_type))
}

fn structural_field_shape(
    field: &StructuralFieldType,
    owner: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<ValueShape, LoweringError> {
    match field {
        StructuralFieldType::Scalar(ScalarType::Boolean) => Ok(ValueShape::integer(1, 1)),
        StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
            let size = integer.bits().div_ceil(8);
            Ok(ValueShape::integer(size, size.next_power_of_two().min(16)))
        }
        StructuralFieldType::Scalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32))
        | StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32) => Ok(ValueShape::float(4)),
        StructuralFieldType::Scalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64))
        | StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary64) => Ok(ValueShape::float(8)),
        StructuralFieldType::ByteSequence(carrier) => byte_sequence_shape(*carrier, owner),
        StructuralFieldType::Structural(nested) => {
            structural_shape(*nested, declarations, cache, active)
        }
        StructuralFieldType::Erased { .. } => {
            // Callers filter erased fields before requesting runtime shape.
            Err(LoweringError::UnknownStructuralType(owner))
        }
    }
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
    scalar_type: semantic_vocabulary::IntegerType,
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

pub(super) fn resolve_structural_projection_path(
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
        let (selected_type, shape, local_offset) = match segment {
            StructuralPathSegment::Field(_) => resolve_structural_field_path(
                structural_type,
                std::slice::from_ref(segment),
                declarations,
                cache,
                active,
            )?,
            StructuralPathSegment::FixedIndex(index) => {
                let declaration = declarations
                    .get(&structural_type)
                    .copied()
                    .ok_or(LoweringError::UnknownStructuralType(structural_type))?;
                let StructuralTypeShape::FixedArray { element, length } = declaration.shape else {
                    return Err(LoweringError::UnknownStructuralType(structural_type));
                };
                if *index >= length {
                    return Err(LoweringError::UnknownStructuralType(structural_type));
                }
                let shape = structural_shape(element, declarations, cache, active)?;
                let stride =
                    checked_align_up_u32(u32::from(shape.byte_size), u32::from(shape.alignment))
                        .ok_or(LoweringError::StructuralTypeTooLarge(root_type))?;
                let offset = u64::from(stride)
                    .checked_mul(*index)
                    .and_then(|offset| u32::try_from(offset).ok())
                    .ok_or(LoweringError::StructuralTypeTooLarge(root_type))?;
                (element, shape, offset)
            }
        };
        total_offset = total_offset
            .checked_add(local_offset)
            .ok_or(LoweringError::StructuralTypeTooLarge(root_type))?;
        structural_type = selected_type;
        selected_shape = Some(shape);
    }
    selected_shape
        .map(|shape| (structural_type, shape, total_offset))
        .ok_or(LoweringError::UnknownStructuralType(root_type))
}

pub(super) fn byte_sequence_shape(
    carrier: terminal_psi::ByteSequenceCarrier,
    structural_type: StructuralTypeId,
) -> Result<ValueShape, LoweringError> {
    let byte_size = match carrier {
        // Current native targets are 64-bit. The semantic carrier
        // deliberately does not retain the physical descriptor fields.
        terminal_psi::ByteSequenceCarrier::BorrowedView => 16_u64,
        terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity } => capacity
            .checked_add(8)
            .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?,
    };
    Ok(ValueShape::integer(
        u16::try_from(byte_size)
            .map_err(|_| LoweringError::StructuralTypeTooLarge(structural_type))?,
        8,
    ))
}

pub(super) fn checked_align_up_u32(value: u32, alignment: u32) -> Option<u32> {
    let remainder = value % alignment;
    if remainder == 0 {
        Some(value)
    } else {
        value.checked_add(alignment - remainder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn structural_type(value: u32) -> StructuralTypeId {
        StructuralTypeId::new(value.into()).expect("structural type")
    }

    fn case(value: u32) -> semantic_vocabulary::StructuralCaseId {
        semantic_vocabulary::StructuralCaseId::new(value.into()).expect("case")
    }

    fn field(value: u32) -> StructuralFieldId {
        StructuralFieldId::new(value.into()).expect("field")
    }

    #[test]
    fn byte_read_sum_has_the_canonical_tag_prefixed_layout() {
        let byte_read = structural_type(1);
        let declaration = StructuralTypeDeclaration {
            id: byte_read,
            identity: "std::ByteRead".into(),
            shape: StructuralTypeShape::Sum {
                cases: vec![
                    terminal_psi::StructuralCaseDeclaration {
                        id: case(1),
                        identity: "std::ByteRead::Eof".into(),
                        fields: Vec::new(),
                    },
                    terminal_psi::StructuralCaseDeclaration {
                        id: case(2),
                        identity: "std::ByteRead::Byte".into(),
                        fields: vec![terminal_psi::StructuralFieldDeclaration {
                            id: field(1),
                            identity: "std::ByteRead::Byte::value".into(),
                            relevance: terminal_psi::BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                                semantic_vocabulary::IntegerType::new(
                                    semantic_vocabulary::IntegerSign::Signed,
                                    32,
                                )
                                .expect("i32"),
                            )),
                        }],
                    },
                ],
            },
        };
        let declarations = BTreeMap::from([(byte_read, &declaration)]);

        let shape = structural_shape(
            byte_read,
            &declarations,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )
        .expect("ByteRead target shape");

        assert_eq!(shape, ValueShape::integer(8, 4));
    }

    #[test]
    fn nested_sums_compose_as_payload_shapes() {
        let inner = structural_type(1);
        let outer = structural_type(2);
        let declarations = [
            StructuralTypeDeclaration {
                id: inner,
                identity: "test::Inner".into(),
                shape: StructuralTypeShape::Sum {
                    cases: vec![terminal_psi::StructuralCaseDeclaration {
                        id: case(1),
                        identity: "test::Inner::Only".into(),
                        fields: Vec::new(),
                    }],
                },
            },
            StructuralTypeDeclaration {
                id: outer,
                identity: "test::Outer".into(),
                shape: StructuralTypeShape::Sum {
                    cases: vec![terminal_psi::StructuralCaseDeclaration {
                        id: case(2),
                        identity: "test::Outer::Nested".into(),
                        fields: vec![terminal_psi::StructuralFieldDeclaration {
                            id: field(1),
                            identity: "test::Outer::Nested::value".into(),
                            relevance: terminal_psi::BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Structural(inner),
                        }],
                    }],
                },
            },
        ];
        let catalog = declarations
            .iter()
            .map(|declaration| (declaration.id, declaration))
            .collect::<BTreeMap<_, _>>();

        let shape = structural_shape(outer, &catalog, &mut BTreeMap::new(), &mut BTreeSet::new())
            .expect("nested sum target shape");

        assert_eq!(shape, ValueShape::integer(8, 4));
    }
}
