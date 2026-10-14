//! Closed byte geometry for reference views. Authored record placements
//! override canonical value geometry; fragmented scalar storage has no direct
//! projection. Pending target applications expose no concrete geometry until
//! the caller supplies admitted target-closed declarations. These queries
//! describe bytes, never access or lifetime.

use semantic_vocabulary::{AuthoredFieldStorage, ScalarType, StructuralTypeId};
use terminal_psi::{
    StructuralFieldDeclaration, StructuralPathSegment, StructuralRecordStorage,
    StructuralTypeDeclaration, StructuralTypeShape,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceStorageProjection {
    pub byte_offset: u64,
    pub logical_scalar: ScalarType,
    pub stored_scalar: ScalarType,
}

/// Geometry and any scalar codec selected by the same complete source path.
/// A projected logical carrier alone cannot reconstruct authored storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceStorageRegion {
    pub byte_offset: u64,
    pub byte_length: u64,
    pub scalar: Option<ReferenceStorageProjection>,
}

fn align(offset: u64, alignment: u64) -> Option<u64> {
    offset
        .checked_add(alignment.checked_sub(1)?)
        .map(|value| value / alignment * alignment)
}

fn scalar_geometry(scalar: ScalarType) -> Option<(u64, u64)> {
    let width = u64::from(crate::reference_storage_byte_width(scalar)?);
    Some((width, width))
}

fn field_geometry<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    field: &StructuralFieldDeclaration,
    active: &mut Vec<StructuralTypeId>,
) -> Option<(u64, u64)> {
    if let Some(scalar) = field.field_type.scalar_type() {
        return scalar_geometry(
            field
                .storage
                .as_ref()
                .and_then(AuthoredFieldStorage::stored_scalar)
                .unwrap_or(scalar),
        );
    }
    let child = field.field_type.as_structural()?;
    geometry(types, child, active)
}

fn geometry<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    active: &mut Vec<StructuralTypeId>,
) -> Option<(u64, u64)> {
    if active.len() >= 128 || active.contains(&root) {
        return None;
    }
    let shape = &types.clone().find(|row| row.id == root)?.shape;
    active.push(root);
    let result = match shape {
        StructuralTypeShape::PrimitiveScalar(scalar) => scalar_geometry(*scalar),
        StructuralTypeShape::Record {
            storage: StructuralRecordStorage::Authored(storage),
            ..
        } => Some((storage.byte_size, storage.byte_alignment)),
        StructuralTypeShape::Record {
            fields,
            storage: StructuralRecordStorage::Packed,
        } => {
            let mut size = 0;
            let mut alignment = 1;
            for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                let (child_size, child_alignment) = field_geometry(types.clone(), field, active)?;
                size = align(size, child_alignment)?.checked_add(child_size)?;
                alignment = alignment.max(child_alignment);
            }
            Some((align(size, alignment)?, alignment))
        }
        StructuralTypeShape::FixedArray { element, length } => {
            let (size, alignment) = geometry(types.clone(), *element, active)?;
            Some((align(size, alignment)?.checked_mul(*length)?, alignment))
        }
        _ => None,
    };
    active.pop();
    result
}

pub fn reference_storage_size<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> Option<u64> {
    reference_storage_layout(types, root).map(|(size, _)| size)
}

/// Contiguous portable storage size and alignment. An unavailable layout is
/// not a zero-size region or permission to invent native pointer geometry.
pub fn reference_storage_layout<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> Option<(u64, u64)> {
    geometry(types, root, &mut Vec::new())
}

/// Resolve an already-captured static path. Runtime selectors are frozen into
/// fixed coordinates by the interpreter before this query; no expression or
/// source declaration is replayed here.
pub fn reference_storage_projection<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<ReferenceStorageProjection> {
    reference_storage_region(types, root, path)?.scalar
}

/// A selected scalar or aggregate's complete contiguous byte region. This is
/// the same path geometry scalar accesses consume, never another path walker.
pub fn reference_storage_region<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<ReferenceStorageRegion> {
    let projection = storage_path(types.clone(), root, path)?;
    let size = match projection.scalar {
        Some((_, stored)) => scalar_geometry(stored)?.0,
        None => reference_storage_size(types, projection.carrier)?,
    };
    Some(ReferenceStorageRegion {
        byte_offset: projection.byte_offset,
        byte_length: size,
        scalar: projection.scalar.map(|(logical_scalar, stored_scalar)| {
            ReferenceStorageProjection {
                byte_offset: projection.byte_offset,
                logical_scalar,
                stored_scalar,
            }
        }),
    })
}

struct StoragePathProjection {
    byte_offset: u64,
    carrier: StructuralTypeId,
    scalar: Option<(ScalarType, ScalarType)>,
}

fn storage_path<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StoragePathProjection> {
    let mut carrier = root;
    let mut byte_offset = 0_u64;
    let mut repeated_stride = None;
    let mut selected_storage = None;
    let mut selected_scalar = None;
    for segment in path {
        if let StructuralPathSegment::RepresentationView {
            structural_type,
            stored_scalar,
        } = segment
        {
            let source_size = match selected_scalar {
                Some(logical) => scalar_geometry(selected_storage.unwrap_or(logical))?.0,
                None => match types.clone().find(|row| row.id == carrier)?.shape {
                    StructuralTypeShape::PrimitiveScalar(logical) => {
                        scalar_geometry(selected_storage.unwrap_or(logical))?.0
                    }
                    _ => reference_storage_size(types.clone(), carrier)?,
                },
            };
            let view = crate::representation_view_declaration(
                types.clone(),
                *structural_type,
                *stored_scalar,
            )?;
            let view_size = match stored_scalar {
                Some(scalar) => scalar_geometry(*scalar)?.0,
                None => reference_storage_size(types.clone(), view.id)?,
            };
            if source_size != view_size {
                return None;
            }
            carrier = view.id;
            selected_storage = *stored_scalar;
            selected_scalar = None;
            repeated_stride = None;
            continue;
        }
        if selected_scalar.is_some() {
            return None;
        }
        let shape = &types.clone().find(|row| row.id == carrier)?.shape;
        match (shape, segment) {
            (
                StructuralTypeShape::Record { fields, storage },
                StructuralPathSegment::Field(identity),
            ) => {
                let field = terminal_psi::live_field_by_identity(fields, *identity)?;
                // A pending record's absent field recipe is not its logical
                // scalar width or a declaration-order byte offset.
                let offset = match storage {
                    terminal_psi::StructuralRecordStorage::PlacedView(_)
                    | terminal_psi::StructuralRecordStorage::PlacedAccessor(_) => return None,
                    StructuralRecordStorage::TargetLayout(_) => return None,
                    StructuralRecordStorage::Authored(_) => {
                        field.storage.as_ref()?.byte_offset()?
                    }
                    StructuralRecordStorage::Packed => {
                        let mut offset = 0;
                        for candidate in fields.iter().filter(|field| !field.relevance.is_erased())
                        {
                            let (size, alignment) =
                                field_geometry(types.clone(), candidate, &mut Vec::new())?;
                            offset = align(offset, alignment)?;
                            if candidate.id == field.id {
                                break;
                            }
                            offset = offset.checked_add(size)?;
                        }
                        offset
                    }
                };
                byte_offset = byte_offset.checked_add(offset)?;
                selected_storage = field
                    .storage
                    .as_ref()
                    .and_then(AuthoredFieldStorage::stored_scalar);
                repeated_stride = field
                    .storage
                    .as_ref()
                    .and_then(AuthoredFieldStorage::element_stride);
                if let Some(logical_scalar) = field.field_type.scalar_type() {
                    selected_scalar = Some(logical_scalar);
                    continue;
                }
                let child = field.field_type.as_structural()?;
                carrier = child;
            }
            (
                StructuralTypeShape::FixedArray { element, length },
                StructuralPathSegment::FixedIndex(index),
            ) if index < length => {
                let (size, alignment) = geometry(types.clone(), *element, &mut Vec::new())?;
                let stride = repeated_stride.take().unwrap_or(align(size, alignment)?);
                byte_offset = byte_offset.checked_add(index.checked_mul(stride)?)?;
                carrier = *element;
                selected_storage = None;
            }
            _ => return None,
        }
    }
    let scalar = match selected_scalar {
        Some(logical) => Some((logical, selected_storage.unwrap_or(logical))),
        None => match types.clone().find(|row| row.id == carrier)?.shape {
            StructuralTypeShape::PrimitiveScalar(logical) => {
                Some((logical, selected_storage.unwrap_or(logical)))
            }
            _ => None,
        },
    };
    Some(StoragePathProjection {
        byte_offset,
        carrier,
        scalar,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{AuthoredRecordStorage, IntegerSign, IntegerType, StructuralFieldId};
    use terminal_psi::{BindingRelevance, StructuralFieldType};

    fn identity(value: u64) -> StructuralTypeId {
        StructuralTypeId::new(value).expect("type identity")
    }
    fn scalar(bits: u16) -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, bits).expect("signed scalar"))
    }
    fn field(
        value: u64,
        field_type: StructuralFieldType,
        storage: Option<AuthoredFieldStorage>,
    ) -> StructuralFieldDeclaration {
        StructuralFieldDeclaration {
            id: StructuralFieldId::new(value).expect("field identity"),
            identity: terminal_psi::TerminalIdentityIndex(4),
            relevance: BindingRelevance::Relevant,
            field_type,
            storage,
        }
    }

    #[test]
    fn canonical_parent_keeps_authored_child_width_and_codec() {
        let types = [
            StructuralTypeDeclaration {
                quotient_carrier: None,
                no_code_affine_cleanup: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: identity(1),
                identity: terminal_psi::TerminalIdentityIndex(3),
                shape: StructuralTypeShape::Record {
                    storage: StructuralRecordStorage::Authored(AuthoredRecordStorage {
                        byte_size: 1,
                        byte_alignment: 1,
                    }),
                    fields: vec![field(
                        1,
                        StructuralFieldType::Scalar(scalar(64)),
                        Some(AuthoredFieldStorage::Scalar {
                            byte_offset: 0,
                            stored_scalar: Some(scalar(8)),
                        }),
                    )],
                },
            },
            StructuralTypeDeclaration {
                quotient_carrier: None,
                no_code_affine_cleanup: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: identity(2),
                identity: terminal_psi::TerminalIdentityIndex(2),
                shape: StructuralTypeShape::Record {
                    storage: StructuralRecordStorage::Packed,
                    fields: vec![field(2, StructuralFieldType::Structural(identity(1)), None)],
                },
            },
        ];
        assert_eq!(reference_storage_size(types.iter(), identity(2)), Some(1));
        assert_eq!(
            reference_storage_projection(
                types.iter(),
                identity(2),
                &[
                    StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(4)),
                    StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(4))
                ]
            ),
            Some(ReferenceStorageProjection {
                byte_offset: 0,
                logical_scalar: scalar(64),
                stored_scalar: scalar(8)
            })
        );
    }

    #[test]
    fn fragmented_fields_do_not_hide_the_record_extent_or_invent_a_scalar_address() {
        let types = [StructuralTypeDeclaration {
            quotient_carrier: None,
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id: identity(1),
            identity: terminal_psi::TerminalIdentityIndex(1),
            shape: StructuralTypeShape::Record {
                storage: StructuralRecordStorage::Authored(AuthoredRecordStorage {
                    byte_size: 1,
                    byte_alignment: 1,
                }),
                fields: vec![field(
                    1,
                    StructuralFieldType::Scalar(scalar(8)),
                    Some(AuthoredFieldStorage::Fragmented(
                        semantic_vocabulary::AuthoredBitFragments::new([
                            semantic_vocabulary::AuthoredBitFragment {
                                container_byte_offset: 0,
                                container_width_bits: 8,
                                destination_lsb: 4,
                                source_lsb: 0,
                                width: 4,
                            },
                            semantic_vocabulary::AuthoredBitFragment {
                                container_byte_offset: 0,
                                container_width_bits: 8,
                                destination_lsb: 0,
                                source_lsb: 4,
                                width: 4,
                            },
                        ]),
                    )),
                )],
            },
        }];
        assert_eq!(reference_storage_size(types.iter(), identity(1)), Some(1));
        assert_eq!(
            reference_storage_projection(
                types.iter(),
                identity(1),
                &[StructuralPathSegment::Field(
                    terminal_psi::TerminalIdentityIndex(4)
                )]
            ),
            None
        );
    }
}
