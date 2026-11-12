/*
  Native structural container geometry, field/array/case byte-offset projection,
  and scalar-field layout resolution (`structural_shape`, `structural_sum_layout`,
  `resolve_structural_projection_path`, `runtime_projection`).

  Derives target byte sizes, alignments, and contiguous storage regions from
  canonical `StructuralTypeDeclaration` tables while preserving empty-array
  element declarations and reference-crossing boundaries.
*/
use crate::LoweringError;
use terminal_semantics::canonical_structural_storage_path;
use terminal_semantics::primitive_projection_declaration;
use terminal_semantics::{primitive_projection_type, primitive_read_projection_type};

fn geometry_error(
    error: calling_conventions::terminal_layout::TerminalLayoutError,
) -> LoweringError {
    use calling_conventions::terminal_layout::TerminalLayoutError as Geometry;
    match error {
        Geometry::RecursiveStructuralType(identity) => {
            LoweringError::RecursiveStructuralType(identity)
        }
        Geometry::UnknownStructuralType(identity) => LoweringError::UnknownStructuralType(identity),
        Geometry::UnclosedTargetLayout(identity) => LoweringError::UnclosedTargetLayout(identity),
        Geometry::UnclosedArrayExtent(identity) => LoweringError::UnclosedArrayExtent(identity),
        Geometry::StructuralTypeTooLarge(identity) => {
            LoweringError::StructuralTypeTooLarge(identity)
        }
        Geometry::EmptyStructuralType(identity) => LoweringError::EmptyStructuralType(identity),
        Geometry::UnsupportedStructuralSum(identity) => {
            LoweringError::UnsupportedStructuralSum(identity)
        }
    }
}
use calling_conventions::ValueShape;
use calling_conventions::terminal_layout::StructuralLayouts;
use semantic_vocabulary::{ScalarType, StructuralTypeId};
use terminal_psi::{StructuralAccess, StructuralPathSegment};
use terminal_psi::{StructuralFieldDeclaration, StructuralFieldType, StructuralTypeShape};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct StorageProjection {
    pub(crate) byte_offset: u32,
    pub(crate) indices: Vec<(semantic_vocabulary::ValueId, u32)>,
}

pub(crate) struct PrimitiveProjection {
    pub(crate) scalar: ScalarType,
    pub(crate) pointer_slots: Vec<StorageProjection>,
    pub(crate) leaf: StorageProjection,
}

impl PrimitiveProjection {
    pub(crate) fn selector_runs(self) -> Vec<(semantic_vocabulary::ValueId, u32)> {
        self.pointer_slots
            .into_iter()
            .chain([self.leaf])
            .flat_map(|region| region.indices)
            .collect()
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn reference_referent_projection(
    mut root: StructuralTypeId,
    mut path: &[StructuralPathSegment],
    requested: StructuralAccess,
    expected: Option<StructuralTypeId>,
    declarations: &StructuralLayouts<'_>,
) -> Result<
    (
        StructuralTypeId,
        ValueShape,
        u32,
        Vec<(semantic_vocabulary::ValueId, u32)>,
    ),
    LoweringError,
> {
    let mut runs = Vec::new();
    while let Some(crossing) = path
        .iter()
        .position(|segment| *segment == StructuralPathSegment::Referent)
    {
        let (carrier, _, _, indices) = runtime_projection(root, &path[..crossing], declarations)?;
        let Some(StructuralTypeShape::Reference { referent, access }) = declarations
            .get(carrier)
            .map(|declaration| &declaration.shape)
        else {
            return Err(LoweringError::UnsupportedStructuralReference(root));
        };
        if !(*access == requested
            || *access == StructuralAccess::MutableBorrow
                && matches!(
                    requested,
                    StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow
                ))
        {
            return Err(LoweringError::UnsupportedStructuralReference(root));
        }
        runs.extend(indices);
        root = *referent;
        path = &path[crossing + 1..];
        if *access == StructuralAccess::WriteOnlyBorrow
            && path.contains(&StructuralPathSegment::Referent)
        {
            return Err(LoweringError::UnsupportedStructuralReference(root));
        }
    }
    let (selected, shape, offset, indices) =
        runtime_borrowed_projection(root, path, requested, expected, declarations)?;
    runs.extend(indices);
    Ok((selected, shape, offset, runs))
}

pub(crate) fn primitive_leaf_projection(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &StructuralLayouts<'_>,
) -> Option<PrimitiveProjection> {
    primitive_access_projection(root, path, declarations, contiguous_primitive_projection)
}

fn primitive_access_projection(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &StructuralLayouts<'_>,
    endpoint: impl Fn(
        StructuralTypeId,
        &[StructuralPathSegment],
        &StructuralLayouts<'_>,
    ) -> Option<(ScalarType, u32, Vec<(semantic_vocabulary::ValueId, u32)>)>,
) -> Option<PrimitiveProjection> {
    let regions = reference_projection_regions(root, path, declarations)?;
    let (scalar, byte_offset, indices) = endpoint(regions.root, regions.path, declarations)?;
    Some(PrimitiveProjection {
        scalar,
        pointer_slots: regions.pointer_slots,
        leaf: StorageProjection {
            byte_offset,
            indices,
        },
    })
}

pub(crate) struct ReferenceProjectionRegions<'path> {
    pub(crate) root: StructuralTypeId,
    pub(crate) path: &'path [StructuralPathSegment],
    pub(crate) pointer_slots: Vec<StorageProjection>,
    pub(crate) readable: bool,
}

impl ReferenceProjectionRegions<'_> {
    pub(crate) fn selector_runs(
        self,
        leaf: Vec<(semantic_vocabulary::ValueId, u32)>,
    ) -> Vec<(semantic_vocabulary::ValueId, u32)> {
        self.pointer_slots
            .into_iter()
            .flat_map(|slot| slot.indices)
            .chain(leaf)
            .collect()
    }
}

pub(crate) fn reference_projection_regions<'path>(
    mut root: StructuralTypeId,
    path: &'path [StructuralPathSegment],
    declarations: &StructuralLayouts<'_>,
) -> Option<ReferenceProjectionRegions<'path>> {
    let mut pointer_slots = Vec::new();
    let mut readable = true;
    let mut segment_start = 0;
    for (position, segment) in path.iter().enumerate() {
        if !matches!(
            segment,
            StructuralPathSegment::Referent
                | StructuralPathSegment::FixedIndex(_)
                | StructuralPathSegment::RuntimeIndex { .. }
        ) {
            continue;
        }
        let selected =
            runtime_descriptor_projection(root, &path[segment_start..position], declarations).ok();
        let Some((carrier, shape, byte_offset, indices)) = selected else {
            if *segment == StructuralPathSegment::Referent {
                return None;
            }
            continue;
        };
        let (next_root, next_start) = match (segment, &declarations.get(carrier)?.shape) {
            (
                StructuralPathSegment::Referent,
                StructuralTypeShape::Reference { referent, access },
            ) => {
                readable &= matches!(
                    access,
                    StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                );
                if !matches!(shape.byte_size, 8 | 16) || shape.alignment != 8 {
                    return None;
                }
                (*referent, position + 1)
            }
            (
                StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. },
                StructuralTypeShape::ElementView { .. },
            ) => {
                if shape.byte_size != 16 || shape.alignment != 8 {
                    return None;
                }
                (carrier, position)
            }
            (
                StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. },
                borrowed,
            ) if borrowed.is_borrowed_byte_carrier() => (carrier, position),
            (StructuralPathSegment::Referent, _) => return None,
            _ => continue,
        };
        pointer_slots.push(StorageProjection {
            byte_offset,
            indices,
        });
        root = next_root;
        segment_start = next_start;
    }
    Some(ReferenceProjectionRegions {
        root,
        path: &path[segment_start..],
        pointer_slots,
        readable,
    })
}

#[allow(clippy::type_complexity)]
fn contiguous_primitive_projection(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &StructuralLayouts<'_>,
) -> Option<(ScalarType, u32, Vec<(semantic_vocabulary::ValueId, u32)>)> {
    if let Some((selector, prefix)) = path.split_last()
        && matches!(
            selector,
            StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. }
        )
        && let Ok((selected, _, owner_offset, mut indices)) =
            runtime_projection(root, prefix, declarations)
        && let StructuralTypeShape::ByteSequence(carrier) = declarations.get(selected)?.shape
    {
        let scalar = primitive_projection_type(declarations.values().copied(), root, path)?;
        let payload_offset: u32 = match carrier {
            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. } => 8,
            terminal_psi::ByteSequenceCarrier::BoundedView { .. }
            | terminal_psi::ByteSequenceCarrier::BorrowedView { .. } => 0,
        };
        let payload_offset = owner_offset.checked_add(payload_offset)?;
        return match selector {
            StructuralPathSegment::FixedIndex(index) => Some((
                scalar,
                payload_offset.checked_add(u32::try_from(*index).ok()?)?,
                indices,
            )),
            StructuralPathSegment::RuntimeIndex { index, .. } => {
                indices.push((*index, 1));
                Some((scalar, payload_offset, indices))
            }
            _ => None,
        };
    }
    if let Some((StructuralPathSegment::Field(identity), carrier_path)) = path.split_last() {
        let (carrier, _, offset, runs) =
            runtime_projection(root, carrier_path, declarations).ok()?;
        if let Some(fields) = declarations.get(carrier)?.shape.direct_fields() {
            let field = terminal_psi::live_field_by_identity(fields, *identity)?;
            let scalar = match field.field_type {
                StructuralFieldType::Scalar(scalar @ ScalarType::IeeeFloat(_)) => Some(scalar),
                StructuralFieldType::IeeeFloat(format) => Some(ScalarType::IeeeFloat(format)),
                _ => None,
            };
            if let Some(scalar) = scalar {
                let field_offset =
                    direct_scalar_field_offset(carrier, field.id, scalar, declarations).ok()?;
                return Some((scalar, offset.checked_add(field_offset)?, runs));
            }
        }
    }
    let (leaf, _, byte_offset, runs) = runtime_projection(root, path, declarations).ok()?;
    match declarations.get(leaf)?.shape {
        StructuralTypeShape::PrimitiveScalar(scalar) => Some((scalar, byte_offset, runs)),
        _ => None,
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn primitive_read_projection(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &StructuralLayouts<'_>,
) -> Option<PrimitiveProjection> {
    let scalar = primitive_read_projection_type(declarations.values().copied(), root, path)?;
    let projection = primitive_access_projection(
        root,
        path,
        declarations,
        contiguous_primitive_read_projection,
    )?;
    (projection.scalar == scalar).then_some(projection)
}

#[allow(clippy::type_complexity)]
fn contiguous_primitive_read_projection(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &StructuralLayouts<'_>,
) -> Option<(ScalarType, u32, Vec<(semantic_vocabulary::ValueId, u32)>)> {
    if let Some(leaf) = contiguous_primitive_projection(root, path, declarations) {
        return Some(leaf);
    }
    let (StructuralPathSegment::Field(_), carrier) = path.split_last()? else {
        return None;
    };
    let field =
        terminal_semantics::runtime_structural_field(declarations.values().copied(), root, path)?;
    let (prefix, namespace) = match carrier.split_last() {
        Some((StructuralPathSegment::Case(_), prefix)) => (prefix, &carrier[carrier.len() - 1..]),
        _ => (carrier, &[][..]),
    };
    let (carrier_type, _, carrier_offset, runs) =
        runtime_projection(root, prefix, declarations).ok()?;
    let scalar = field.field_type.scalar_type()?;
    let (field_offset, _) = if namespace.is_empty() {
        direct_scalar_field_geometry(carrier_type, field.id, scalar, declarations).ok()?
    } else {
        static_field_geometry(carrier_type, namespace, field.id, scalar, declarations).ok()?
    };
    Some((scalar, carrier_offset.checked_add(field_offset)?, runs))
}

mod scalar_fields {
    use super::LoweringError;
    use super::structural_sum_layout;
    use calling_conventions::terminal_layout::StructuralLayouts;
    use semantic_vocabulary::{ScalarType, StructuralFieldId, StructuralTypeId};
    use terminal_psi::{StructuralFieldType, StructuralTypeShape};

    pub(crate) fn static_atomic_geometry(
        root: StructuralTypeId,
        path: &[terminal_psi::StructuralPathSegment],
        expected: ScalarType,
        declarations: &StructuralLayouts<'_>,
    ) -> Result<(u32, u32), LoweringError> {
        let invalid = || LoweringError::UnknownStructuralType(root);
        terminal_semantics::atomic_cell_type(declarations.values().copied(), root, path)
            .filter(|cell| cell.scalar_type() == expected)
            .ok_or_else(invalid)?;
        if let Some((terminal_psi::StructuralPathSegment::Field(_), prefix)) = path.split_last() {
            let field = terminal_semantics::runtime_structural_field(
                declarations.values().copied(),
                root,
                path,
            )
            .ok_or_else(invalid)?;
            return static_field_geometry(root, prefix, field.id, expected, declarations);
        }
        let (selected, shape, offset, indexes) =
            super::runtime_projection(root, path, declarations)?;
        if !indexes.is_empty()
            || !matches!(declarations.get(selected).map(|row| &row.shape),
            Some(StructuralTypeShape::AtomicCell(cell)) if cell.scalar_type() == expected)
        {
            return Err(invalid());
        }
        Ok((offset, shape.byte_size))
    }

    pub(crate) fn static_field_geometry(
        root: StructuralTypeId,
        path: &[terminal_psi::StructuralPathSegment],
        field: StructuralFieldId,
        expected: ScalarType,
        declarations: &StructuralLayouts<'_>,
    ) -> Result<(u32, u32), LoweringError> {
        let invalid = || LoweringError::UnknownStructuralType(root);
        let (_, declaration) = terminal_semantics::static_field_projection(
            declarations.values().copied(),
            root,
            path,
            field,
        )
        .ok_or_else(invalid)?;
        if declaration.field_type.scalar_type() != Some(expected)
            || matches!(
                declaration.storage.as_ref(),
                Some(semantic_vocabulary::AuthoredFieldStorage::Fragmented(_))
            )
        {
            return Err(invalid());
        }
        let (prefix, selected_case) = match path.split_last() {
            Some((terminal_psi::StructuralPathSegment::Case(case), prefix)) => {
                (prefix, Some(*case))
            }
            _ => (path, None),
        };
        let (carrier, _, offset, indices) = super::runtime_projection(root, prefix, declarations)?;
        if !indices.is_empty() {
            return Err(invalid());
        }
        let (displacement, bytes) = if let Some(case) = selected_case {
            let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                &declarations.get(carrier).ok_or_else(invalid)?.shape
            else {
                return Err(invalid());
            };
            let case_position = cases
                .iter()
                .position(|candidate| candidate.id == case)
                .ok_or_else(invalid)?;
            let field_position = cases[case_position]
                .fields
                .iter()
                .filter(|candidate| !candidate.relevance.is_erased())
                .position(|candidate| candidate.id == field)
                .ok_or_else(invalid)?;
            let layout = super::structural_sum_layout(carrier, declarations)?;
            let placed = layout.cases[case_position].fields[field_position];
            (placed.byte_offset, placed.shape.byte_size)
        } else {
            direct_scalar_field_geometry(carrier, field, expected, declarations)?
        };
        Ok((offset.checked_add(displacement).ok_or_else(invalid)?, bytes))
    }
    pub(crate) fn direct_scalar_field_offset(
        structural_type: StructuralTypeId,
        field: StructuralFieldId,
        expected_type: ScalarType,
        declarations: &StructuralLayouts<'_>,
    ) -> Result<u32, LoweringError> {
        if declarations
            .get(structural_type)
            .is_some_and(|declaration| {
                let fields = match &declaration.shape {
                    terminal_psi::record_or_mixed_shape_pattern!(fields) => fields,
                    _ => return false,
                };
                fields.iter().any(|candidate| {
                    candidate.id == field
                        && matches!(
                            candidate.storage.as_ref(),
                            Some(semantic_vocabulary::AuthoredFieldStorage::Fragmented(_))
                        )
                })
            })
        {
            return Err(LoweringError::UnknownStructuralType(structural_type));
        }
        direct_scalar_field_geometry(structural_type, field, expected_type, declarations)
            .map(|(offset, _)| offset)
    }

    pub(crate) fn direct_scalar_field_geometry(
        structural_type: StructuralTypeId,
        field: StructuralFieldId,
        expected_type: ScalarType,
        declarations: &StructuralLayouts<'_>,
    ) -> Result<(u32, u32), LoweringError> {
        let declaration = declarations
            .get(structural_type)
            .copied()
            .ok_or(LoweringError::UnknownStructuralType(structural_type))?;
        if let StructuralTypeShape::Mixed { fields, .. } = &declaration.shape {
            let (position, candidate) = fields
                .iter()
                .filter(|candidate| !candidate.relevance.is_erased())
                .enumerate()
                .find(|(_, candidate)| candidate.id == field)
                .ok_or(LoweringError::UnknownStructuralType(structural_type))?;
            if candidate.field_type.scalar_type() != Some(expected_type) {
                return Err(LoweringError::UnknownStructuralType(structural_type));
            }
            return structural_sum_layout(structural_type, declarations)?
                .common_fields
                .get(position)
                .map(|common| (common.byte_offset, common.shape.byte_size))
                .ok_or(LoweringError::UnknownStructuralType(structural_type));
        }
        let Some(fields) = declaration.shape.record_fields() else {
            return Err(LoweringError::UnknownStructuralType(structural_type));
        };
        let mut offset = 0_u32;
        for candidate in fields
            .iter()
            .filter(|candidate| !candidate.relevance.is_erased())
        {
            if matches!(candidate.field_type, StructuralFieldType::Erased { .. }) {
                continue;
            }
            if candidate.storage.is_some() && candidate.id != field {
                continue;
            }
            if candidate.id == field
                && let Some(semantic_vocabulary::AuthoredFieldStorage::Fragmented(fragments)) =
                    candidate.storage.as_ref()
            {
                if matches!(
                    candidate.field_type,
                    StructuralFieldType::Atomic(_) | StructuralFieldType::AtomicBoundedInteger(_)
                ) || candidate.field_type.scalar_type() != Some(expected_type)
                {
                    return Err(LoweringError::UnknownStructuralType(structural_type));
                }
                let (start, width) = fragments
                    .byte_envelope()
                    .ok_or(LoweringError::UnknownStructuralType(structural_type))?;
                return Ok((
                    u32::try_from(start)
                        .map_err(|_| LoweringError::StructuralTypeTooLarge(structural_type))?,
                    u32::try_from(width)
                        .map_err(|_| LoweringError::StructuralTypeTooLarge(structural_type))?,
                ));
            }
            let shape =
                super::structural_field_storage_shape(candidate, structural_type, declarations)?;
            offset = super::structural_field_offset(
                candidate,
                offset,
                shape.alignment,
                structural_type,
            )?;
            if candidate.id == field {
                return (candidate.field_type.scalar_type() == Some(expected_type))
                    .then_some((offset, shape.byte_size))
                    .ok_or(LoweringError::UnknownStructuralType(structural_type));
            }
            offset = offset
                .checked_add(shape.byte_size)
                .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
        }
        Err(LoweringError::UnknownStructuralType(structural_type))
    }
}
pub(crate) use scalar_fields::{
    direct_scalar_field_geometry, direct_scalar_field_offset, static_atomic_geometry,
    static_field_geometry,
};

pub(crate) fn root_array_projection_metadata(
    root: StructuralTypeId,
    declarations: &StructuralLayouts<'_>,
) -> Result<(Option<u64>, Option<u32>), LoweringError> {
    let declaration = declarations
        .get(root)
        .ok_or(LoweringError::UnknownStructuralType(root))?;
    let StructuralTypeShape::FixedArray {
        element,
        length: terminal_psi::ArrayExtent::Literal(length),
    } = declaration.shape
    else {
        return Ok((None, None));
    };
    let shape = structural_shape(element, declarations)?;
    let stride = checked_align_up_u32(shape.byte_size, u32::from(shape.alignment))
        .ok_or(LoweringError::StructuralTypeTooLarge(root))?;
    Ok((Some(length), Some(stride)))
}

pub(crate) fn structural_shape(
    structural_type: StructuralTypeId,
    declarations: &StructuralLayouts<'_>,
) -> Result<ValueShape, LoweringError> {
    declarations.shape(structural_type).map_err(geometry_error)
}

pub(crate) fn scalar_storage_type(
    structural_type: StructuralTypeId,
    declarations: &StructuralLayouts<'_>,
) -> Option<ScalarType> {
    declarations.scalar_storage_type(structural_type)
}

pub(crate) fn structural_sum_layout(
    structural_type: StructuralTypeId,
    declarations: &StructuralLayouts<'_>,
) -> Result<calling_conventions::ConventionalSumLayout, LoweringError> {
    declarations
        .sum_layout(structural_type)
        .map_err(geometry_error)
}

fn structural_field_storage_shape(
    field: &terminal_psi::StructuralFieldDeclaration,
    owner: StructuralTypeId,
    declarations: &StructuralLayouts<'_>,
) -> Result<ValueShape, LoweringError> {
    if matches!(
        field.storage.as_ref(),
        Some(semantic_vocabulary::AuthoredFieldStorage::Fragmented(_))
    ) {
        return Err(LoweringError::UnknownStructuralType(owner));
    }
    if let Some(stored) = field
        .storage
        .as_ref()
        .and_then(|storage| storage.stored_scalar())
    {
        let codec = runtime_abi::ScalarStorageCodec::from_scalar(stored)
            .ok_or(LoweringError::UnknownStructuralType(owner))?;
        return Ok(ValueShape::integer(u32::from(codec.byte_size()), 1));
    }
    structural_field_shape(&field.field_type, owner, declarations)
}

fn structural_field_offset(
    field: &terminal_psi::StructuralFieldDeclaration,
    conventional_offset: u32,
    alignment: u16,
    owner: StructuralTypeId,
) -> Result<u32, LoweringError> {
    match field.storage.as_ref() {
        Some(storage) => storage
            .byte_offset()
            .and_then(|offset| u32::try_from(offset).ok())
            .ok_or(LoweringError::UnknownStructuralType(owner)),
        None => checked_align_up_u32(conventional_offset, u32::from(alignment))
            .ok_or(LoweringError::StructuralTypeTooLarge(owner)),
    }
}

fn structural_field_shape(
    field: &StructuralFieldType,
    owner: StructuralTypeId,
    declarations: &StructuralLayouts<'_>,
) -> Result<ValueShape, LoweringError> {
    declarations
        .field_shape(field, owner)
        .map_err(geometry_error)
}

fn field_geometry<'types>(
    structural_type: StructuralTypeId,
    segment: &StructuralPathSegment,
    declarations: &StructuralLayouts<'types>,
) -> Result<
    (
        &'types StructuralFieldDeclaration,
        ValueShape,
        u32,
        Option<u64>,
    ),
    LoweringError,
> {
    let root_type = structural_type;
    let StructuralPathSegment::Field(identity) = segment else {
        return Err(LoweringError::UnknownStructuralType(structural_type));
    };
    let declaration = declarations
        .get(structural_type)
        .copied()
        .ok_or(LoweringError::UnknownStructuralType(structural_type))?;
    let Some(fields) = declaration.shape.direct_fields() else {
        return Err(LoweringError::UnknownStructuralType(structural_type));
    };
    let common_layout = if matches!(declaration.shape, StructuralTypeShape::Mixed { .. }) {
        Some(structural_sum_layout(structural_type, declarations)?)
    } else {
        None
    };
    let mut local_offset = 0_u32;
    for (field_position, field) in fields
        .iter()
        .filter(|field| !field.relevance.is_erased())
        .enumerate()
    {
        if matches!(field.field_type, StructuralFieldType::Erased { .. }) {
            continue;
        }
        if field.storage.is_some() && field.identity != *identity {
            continue;
        }
        let field_shape = structural_field_storage_shape(field, structural_type, declarations)?;
        local_offset = match &common_layout {
            Some(layout) => {
                layout
                    .common_fields
                    .get(field_position)
                    .ok_or(LoweringError::UnknownStructuralType(structural_type))?
                    .byte_offset
            }
            None => structural_field_offset(field, local_offset, field_shape.alignment, root_type)?,
        };
        if field.identity == *identity {
            let repeated_stride = field
                .storage
                .as_ref()
                .and_then(semantic_vocabulary::AuthoredFieldStorage::element_stride);
            return Ok((field, field_shape, local_offset, repeated_stride));
        }
        local_offset = local_offset
            .checked_add(field_shape.byte_size)
            .ok_or(LoweringError::StructuralTypeTooLarge(root_type))?;
    }
    Err(LoweringError::UnknownStructuralType(root_type))
}

fn byte_sequence_field_geometry(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &StructuralLayouts<'_>,
) -> Result<Option<(u32, terminal_psi::ByteSequenceCarrier)>, LoweringError> {
    if !matches!(path.last(), Some(StructuralPathSegment::Field(_))) {
        return Ok(None);
    }
    let (selected, _, offset, indices) =
        runtime_storage_projection(structural_type, path, declarations)?;
    if !indices.is_empty() {
        return Err(LoweringError::UnsupportedStructuralReference(
            structural_type,
        ));
    }
    Ok(match selected {
        ProjectedStorage::Field(field) => field
            .field_type
            .byte_sequence_carrier()
            .map(|carrier| (offset, carrier)),
        ProjectedStorage::Declaration(_) => None,
    })
}

pub(crate) fn bounded_byte_field_geometry(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &StructuralLayouts<'_>,
) -> Result<Option<(u32, u64)>, LoweringError> {
    byte_sequence_field_geometry(structural_type, path, declarations).map(|geometry| {
        geometry.and_then(|(offset, carrier)| match carrier {
            terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity } => {
                Some((offset, capacity))
            }
            terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
            | terminal_psi::ByteSequenceCarrier::BoundedView { .. } => None,
        })
    })
}

pub(crate) fn borrowed_view_field_offset(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &StructuralLayouts<'_>,
) -> Result<Option<u32>, LoweringError> {
    byte_sequence_field_geometry(structural_type, path, declarations).map(|geometry| {
        geometry.and_then(|(offset, carrier)| match carrier {
            terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
            | terminal_psi::ByteSequenceCarrier::BoundedView { .. } => Some(offset),
            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. } => None,
        })
    })
}

pub(crate) fn case_projection(
    root: StructuralTypeId,
    path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
    declarations: &StructuralLayouts<'_>,
) -> Result<(StructuralTypeId, ValueShape, u32), LoweringError> {
    let invalid = || LoweringError::UnsupportedStructuralSum(root);
    let endpoint =
        terminal_semantics::canonical_case_leaf_type(declarations.values().copied(), root, path)
            .ok_or_else(invalid)?;
    let storage_path =
        canonical_structural_storage_path(declarations.values().copied(), root, path)
            .ok_or_else(invalid)?;
    let (selected, shape, offset, indices) =
        runtime_leaf_copy_projection(root, &storage_path, endpoint, declarations)?;
    if selected != endpoint || !indices.is_empty() {
        return Err(invalid());
    }
    Ok((endpoint, shape, offset))
}

pub(crate) fn resolve_structural_projection_path(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &StructuralLayouts<'_>,
) -> Result<(StructuralTypeId, ValueShape, u32), LoweringError> {
    let (selected, shape, offset, indices) = storage_projection_path(
        structural_type,
        path,
        declarations,
        None,
        StructuralFieldType::canonical_leaf_type,
    )?;
    if !indices.is_empty() {
        return Err(LoweringError::UnsupportedStructuralReference(
            structural_type,
        ));
    }
    Ok((selected, shape, offset))
}

fn storage_projection_path(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &StructuralLayouts<'_>,

    expected: Option<StructuralTypeId>,
    leaf_type: impl Fn(&StructuralFieldType) -> Option<StructuralFieldType>,
) -> Result<RuntimeProjection, LoweringError> {
    let (selected, shape, offset, indices) =
        runtime_storage_projection(structural_type, path, declarations)?;
    Ok((
        selected.declaration(structural_type, expected, declarations, leaf_type)?,
        shape,
        offset,
        indices,
    ))
}

#[derive(Clone, Copy)]
pub(crate) enum ProjectedStorage<'types> {
    Declaration(StructuralTypeId),
    Field(&'types StructuralFieldDeclaration),
}

impl ProjectedStorage<'_> {
    pub(crate) fn declaration(
        self,
        root: StructuralTypeId,
        expected: Option<StructuralTypeId>,
        declarations: &StructuralLayouts<'_>,
        leaf_type: impl Fn(&StructuralFieldType) -> Option<StructuralFieldType>,
    ) -> Result<StructuralTypeId, LoweringError> {
        let leaf = match self {
            Self::Declaration(identity) => return Ok(identity),
            Self::Field(field) => field,
        };
        if let StructuralFieldType::Structural(identity) = leaf.field_type {
            return Ok(identity);
        }
        terminal_semantics::inline_field_presentation(
            declarations.values().copied(),
            leaf,
            expected,
            leaf_type(&leaf.field_type),
        )
        .ok_or(LoweringError::UnknownStructuralType(
            expected.unwrap_or(root),
        ))
    }

    pub(crate) fn shape<'types>(
        self,
        declarations: &StructuralLayouts<'types>,
    ) -> Option<std::borrow::Cow<'types, StructuralTypeShape>> {
        match self {
            Self::Declaration(identity) => declarations
                .get(identity)
                .map(|row| std::borrow::Cow::Borrowed(&row.shape)),
            Self::Field(field) if field.field_type.as_structural().is_some() => declarations
                .get(field.field_type.as_structural()?)
                .map(|row| std::borrow::Cow::Borrowed(&row.shape)),
            Self::Field(field) => field
                .field_type
                .byte_sequence_carrier()
                .map(|carrier| std::borrow::Cow::Owned(StructuralTypeShape::ByteSequence(carrier))),
        }
    }
}

type RuntimeStorageProjection<'types> = (
    ProjectedStorage<'types>,
    ValueShape,
    u32,
    Vec<(semantic_vocabulary::ValueId, u32)>,
);

pub(crate) fn runtime_storage_projection<'types>(
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &StructuralLayouts<'types>,
) -> Result<RuntimeStorageProjection<'types>, LoweringError> {
    let root_type = structural_type;
    let mut selected_storage = ProjectedStorage::Declaration(structural_type);
    let mut total_offset = 0_u32;
    let mut selected_shape = None;
    let mut indices = Vec::new();
    let mut repeated_stride = None;
    let mut segments = path.iter();
    while let Some(segment) = segments.next() {
        structural_type = match selected_storage {
            ProjectedStorage::Declaration(identity) => identity,
            ProjectedStorage::Field(field) => field
                .field_type
                .as_structural()
                .ok_or(LoweringError::UnknownStructuralType(root_type))?,
        };
        let (selected_type, shape, local_offset) = match segment {
            StructuralPathSegment::AllElements => {
                return Err(LoweringError::UnknownStructuralType(structural_type));
            }
            StructuralPathSegment::Case(case) => {
                let invalid = || LoweringError::UnsupportedStructuralSum(structural_type);
                let declaration = declarations.get(structural_type).ok_or_else(invalid)?;
                let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                    &declaration.shape
                else {
                    return Err(invalid());
                };
                let case_position = cases
                    .iter()
                    .position(|candidate| candidate.id == *case)
                    .ok_or_else(invalid)?;
                let Some(StructuralPathSegment::Field(identity)) = segments.next() else {
                    return Err(invalid());
                };
                let (field_position, field) =
                    terminal_psi::live_field_entry(&cases[case_position].fields, *identity)
                        .ok_or_else(invalid)?;
                repeated_stride = field
                    .storage
                    .as_ref()
                    .and_then(semantic_vocabulary::AuthoredFieldStorage::element_stride);
                let layout = structural_sum_layout(structural_type, declarations)?;
                (
                    ProjectedStorage::Field(field),
                    structural_field_storage_shape(field, structural_type, declarations)?,
                    layout.cases[case_position].fields[field_position].byte_offset,
                )
            }
            StructuralPathSegment::RepresentationView { .. }
            | StructuralPathSegment::Referent
            | StructuralPathSegment::FixedRange { .. } => {
                return Err(LoweringError::UnsupportedStructuralReference(
                    structural_type,
                ));
            }
            StructuralPathSegment::Field(_) => {
                let (selected, shape, offset, stride) =
                    field_geometry(structural_type, segment, declarations)?;
                repeated_stride = stride;
                (ProjectedStorage::Field(selected), shape, offset)
            }
            StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. } => {
                let declaration = declarations
                    .get(structural_type)
                    .copied()
                    .ok_or(LoweringError::UnknownStructuralType(structural_type))?;
                let element = match &declaration.shape {
                    StructuralTypeShape::FixedArray {
                        element,
                        length: terminal_psi::ArrayExtent::Literal(length),
                    } => {
                        if matches!(segment, StructuralPathSegment::FixedIndex(index) if index >= length)
                        {
                            return Err(LoweringError::UnknownStructuralType(structural_type));
                        }
                        *element
                    }
                    StructuralTypeShape::ElementView { element, .. } => *element,
                    _ => return Err(LoweringError::UnknownStructuralType(structural_type)),
                };
                let shape = structural_shape(element, declarations)?;
                let stride = match repeated_stride.take() {
                    Some(stride) => u32::try_from(stride)
                        .map_err(|_| LoweringError::StructuralTypeTooLarge(root_type))?,
                    None => checked_align_up_u32(shape.byte_size, u32::from(shape.alignment))
                        .ok_or(LoweringError::StructuralTypeTooLarge(root_type))?,
                };
                let offset = match segment {
                    StructuralPathSegment::FixedIndex(index) => u64::from(stride)
                        .checked_mul(*index)
                        .and_then(|offset| u32::try_from(offset).ok())
                        .ok_or(LoweringError::StructuralTypeTooLarge(root_type))?,
                    StructuralPathSegment::RuntimeIndex { index, .. } => {
                        indices.push((*index, stride));
                        0
                    }
                    _ => return Err(LoweringError::UnknownStructuralType(structural_type)),
                };
                (ProjectedStorage::Declaration(element), shape, offset)
            }
        };
        total_offset = total_offset
            .checked_add(local_offset)
            .ok_or(LoweringError::StructuralTypeTooLarge(root_type))?;
        selected_storage = selected_type;
        selected_shape = Some(shape);
    }
    let shape = match selected_shape {
        Some(shape) => shape,
        None => structural_shape(structural_type, declarations)?,
    };
    Ok((selected_storage, shape, total_offset, indices))
}

type RuntimeProjection = (
    StructuralTypeId,
    ValueShape,
    u32,
    Vec<(semantic_vocabulary::ValueId, u32)>,
);

#[allow(clippy::type_complexity)]
pub(crate) fn runtime_descriptor_projection(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &StructuralLayouts<'_>,
) -> Result<RuntimeProjection, LoweringError> {
    runtime_projection_with_leaf_type(root, path, declarations, None, |field| {
        field
            .byte_sequence_carrier()
            .map(|carrier| {
                StructuralFieldType::ByteSequence(match carrier {
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. } => {
                        terminal_psi::ByteSequenceCarrier::BorrowedView { access: None }
                    }
                    terminal_psi::ByteSequenceCarrier::BoundedView { capacity, .. } => {
                        terminal_psi::ByteSequenceCarrier::BoundedView {
                            access: None,
                            capacity,
                        }
                    }
                    owned => owned,
                })
            })
            .or_else(|| field.canonical_leaf_type())
    })
}

pub(crate) fn runtime_projection(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &StructuralLayouts<'_>,
) -> Result<
    (
        StructuralTypeId,
        ValueShape,
        u32,
        Vec<(semantic_vocabulary::ValueId, u32)>,
    ),
    LoweringError,
> {
    runtime_projection_with_leaf_type(
        structural_type,
        path,
        declarations,
        None,
        StructuralFieldType::canonical_leaf_type,
    )
}

pub(crate) fn runtime_leaf_copy_projection(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    expected: StructuralTypeId,
    declarations: &StructuralLayouts<'_>,
) -> Result<RuntimeProjection, LoweringError> {
    let expected = declarations
        .get(expected)
        .copied()
        .ok_or(LoweringError::UnknownStructuralType(expected))?;
    let regions = reference_projection_regions(structural_type, path, declarations)
        .filter(|regions| regions.readable)
        .ok_or(LoweringError::UnsupportedStructuralReference(
            structural_type,
        ))?;
    let (selected, shape, byte_offset, leaf_indices) = runtime_projection_with_leaf_type(
        regions.root,
        regions.path,
        declarations,
        Some(expected.id),
        |field| field.leaf_copy_type(Some(expected)),
    )?;
    let indices = regions.selector_runs(leaf_indices);
    Ok((selected, shape, byte_offset, indices))
}

pub(crate) fn runtime_borrowed_projection(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    access: StructuralAccess,
    expected: Option<StructuralTypeId>,
    declarations: &StructuralLayouts<'_>,
) -> Result<RuntimeProjection, LoweringError> {
    let regions = reference_projection_regions(structural_type, path, declarations).ok_or(
        LoweringError::UnsupportedStructuralReference(structural_type),
    )?;
    let expected = expected
        .map(|identity| {
            declarations
                .get(identity)
                .copied()
                .ok_or(LoweringError::UnknownStructuralType(identity))
        })
        .transpose()?;
    let (selected, shape, offset, leaf_indices) = runtime_projection_with_leaf_type(
        regions.root,
        regions.path,
        declarations,
        expected.map(|declaration| declaration.id),
        |field| terminal_semantics::borrowed_field_presentation(field, access, expected),
    )?;
    let indices = regions.selector_runs(leaf_indices);
    Ok((selected, shape, offset, indices))
}

pub(crate) fn storage_address_indices(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &StructuralLayouts<'_>,
) -> Result<Vec<(semantic_vocabulary::ValueId, u32)>, LoweringError> {
    let invalid = || LoweringError::UnsupportedStructuralReference(structural_type);
    let regions =
        reference_projection_regions(structural_type, path, declarations).ok_or_else(invalid)?;
    let leaf_indices = if !regions.path.is_empty()
        && matches!(
            declarations.get(regions.root).map(|row| &row.shape),
            Some(StructuralTypeShape::ByteSequence(_))
        ) {
        contiguous_primitive_projection(regions.root, regions.path, declarations)
            .ok_or_else(invalid)?
            .2
    } else {
        runtime_storage_projection(regions.root, regions.path, declarations)?.3
    };
    Ok(regions.selector_runs(leaf_indices))
}

fn runtime_projection_with_leaf_type(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &StructuralLayouts<'_>,

    expected: Option<StructuralTypeId>,
    leaf_type: impl Fn(&StructuralFieldType) -> Option<StructuralFieldType>,
) -> Result<RuntimeProjection, LoweringError> {
    if !path.is_empty()
        && matches!(
            declarations.get(structural_type).map(|row| &row.shape),
            Some(StructuralTypeShape::ByteSequence(_))
        )
    {
        let (scalar, byte_offset, indices) =
            contiguous_primitive_projection(structural_type, path, declarations).ok_or(
                LoweringError::UnsupportedStructuralReference(structural_type),
            )?;
        let endpoint = match expected {
            Some(expected) => {
                let declaration = declarations
                    .get(expected)
                    .ok_or(LoweringError::UnsupportedStructuralReference(expected))?;
                let contract = leaf_type(&StructuralFieldType::Scalar(scalar))
                    .ok_or(LoweringError::UnsupportedStructuralReference(expected))?;
                if !contract.declares_leaf(declaration) {
                    return Err(LoweringError::UnsupportedStructuralReference(expected));
                }
                expected
            }
            None => primitive_projection_declaration(
                declarations.values().copied(),
                structural_type,
                path,
            )
            .ok_or(LoweringError::UnsupportedStructuralReference(
                structural_type,
            ))?,
        };
        let shape = structural_shape(endpoint, declarations)?;
        return Ok((endpoint, shape, byte_offset, indices));
    }
    storage_projection_path(structural_type, path, declarations, expected, leaf_type)
}

pub(crate) fn byte_sequence_shape(
    carrier: terminal_psi::ByteSequenceCarrier,
    structural_type: StructuralTypeId,
) -> Result<ValueShape, LoweringError> {
    calling_conventions::terminal_layout::byte_sequence_shape(carrier, structural_type)
        .map_err(geometry_error)
}

pub(crate) fn checked_align_up_u32(value: u32, alignment: u32) -> Option<u32> {
    calling_conventions::terminal_layout::checked_align_up_u32(value, alignment)
}

#[cfg(test)]
mod placed_geometry_tests {
    use super::*;
    use semantic_vocabulary::{AuthoredFieldStorage, AuthoredRecordStorage, StructuralFieldId};
    use terminal_psi::{
        StructuralFieldDeclaration, StructuralRecordStorage, StructuralTypeDeclaration,
        TerminalIdentityIndex,
    };

    #[test]
    fn typed_byte_element_loan_retains_exact_expected_carrier() {
        let integer =
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
                .unwrap();
        let types = [
            StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
                capacity: 6,
            }),
            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)),
            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)),
            StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        ]
        .into_iter()
        .enumerate()
        .map(|(position, shape)| StructuralTypeDeclaration {
            requires: Vec::new(),
            id: StructuralTypeId::new(position as u64 + 1).unwrap(),
            identity: TerminalIdentityIndex(position as u32 + 1),
            owns_linear_claim: false,
            copyable: true,
            no_code_affine_cleanup: true,
            descriptor_cleanup: false,
            quotient_carrier: None,
            scalar_restriction: None,
            shape,
        })
        .collect::<Vec<_>>();
        let declarations = StructuralLayouts::new(&types).unwrap();
        let root = types[0].id;
        let path = [StructuralPathSegment::FixedIndex(2)];
        let (selected, _, offset, _) = reference_referent_projection(
            root,
            &path,
            StructuralAccess::SharedBorrow,
            Some(types[2].id),
            &declarations,
        )
        .unwrap();
        assert_eq!(selected, types[2].id);
        assert_eq!(offset, 10);
        for wrong in [types[3].id, StructuralTypeId::new(99).unwrap()] {
            assert!(
                reference_referent_projection(
                    root,
                    &path,
                    StructuralAccess::SharedBorrow,
                    Some(wrong),
                    &declarations
                )
                .is_err()
            );
        }
    }

    #[test]
    fn placed_container_geometry_preserves_offset_without_scalar_authority() {
        let root = StructuralTypeId::new(1).unwrap();
        let accessor = StructuralTypeId::new(2).unwrap();
        let field = StructuralFieldId::new(1).unwrap();
        let accessor_declaration = StructuralTypeDeclaration {
            requires: Vec::new(),
            quotient_carrier: None,
            id: accessor,
            identity: TerminalIdentityIndex(2),
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            copyable: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            shape: StructuralTypeShape::Record {
                fields: vec![],
                storage: StructuralRecordStorage::Accessor(AuthoredRecordStorage {
                    byte_size: 4,
                    byte_alignment: 1,
                }),
            },
        };
        let root_declaration = StructuralTypeDeclaration {
            requires: Vec::new(),
            quotient_carrier: None,
            id: root,
            identity: TerminalIdentityIndex(1),
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            copyable: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    inline_carrier: None,
                    id: field,
                    identity: TerminalIdentityIndex(3),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(accessor),
                    storage: Some(AuthoredFieldStorage::Direct { byte_offset: 4 }),
                }],
                storage: StructuralRecordStorage::AccessView(AuthoredRecordStorage {
                    byte_size: 8,
                    byte_alignment: 4,
                }),
            },
        };
        let declarations =
            StructuralLayouts::new([&root_declaration, &accessor_declaration]).unwrap();
        let (selected, shape, offset, indices) = runtime_projection(
            root,
            &[StructuralPathSegment::Field(TerminalIdentityIndex(3))],
            &declarations,
        )
        .unwrap();
        assert_eq!(selected, accessor);
        assert_eq!(shape, ValueShape::integer(4, 1));
        assert_eq!(offset, 4);
        assert!(indices.is_empty());
        assert!(
            primitive_leaf_projection(
                root,
                &[StructuralPathSegment::Field(TerminalIdentityIndex(3))],
                &declarations
            )
            .is_none()
        );
        assert!(!terminal_semantics::zeroed_structural_shape(
            declarations.values().copied(),
            root
        ));
        assert!(!terminal_semantics::zeroed_structural_shape(
            declarations.values().copied(),
            accessor
        ));
    }
}
