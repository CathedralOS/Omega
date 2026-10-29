//! Closed byte geometry for reference views. Authored record placements
//! override canonical value geometry; fragmented scalar storage has no direct
//! projection. Pending target applications expose no concrete geometry until
//! the caller supplies admitted target-closed declarations. These queries
//! describe bytes, never access or lifetime.

use semantic_vocabulary::{
    AuthoredFieldStorage, ScalarApplicationId, ScalarType, StructuralTypeId, TargetCarrierLayout,
};
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
    carriers: Option<TargetCarrierLayout>,
    extents: &mut impl FnMut(ScalarApplicationId) -> Result<Option<u64>, StorageGeometryError>,
) -> Result<Option<(u64, u64)>, StorageGeometryError> {
    if let Some(scalar) = field.field_type.scalar_type() {
        return scalar_geometry(
            field
                .storage
                .as_ref()
                .and_then(AuthoredFieldStorage::stored_scalar)
                .unwrap_or(scalar),
        )
        .map(Some)
        .ok_or(StorageGeometryError::UnsupportedScalar(scalar));
    }
    if let Some(carrier) = field.field_type.byte_sequence_carrier() {
        return byte_sequence_geometry(carrier, carriers).map(Some);
    }
    let child = field
        .field_type
        .as_structural()
        .ok_or(StorageGeometryError::UnsupportedField)?;
    geometry(types, child, active, carriers, extents)
}

/// A missing application is a dependency, never a zero element count. The
/// caller owns evaluation and admission; byte geometry only consumes the exact
/// count supplied for the catalog coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageGeometryError {
    MissingType(StructuralTypeId),
    RecursiveType(StructuralTypeId),
    NestingLimit,
    UnsupportedType(StructuralTypeId),
    UnsupportedScalar(ScalarType),
    UnsupportedField,
    UnsupportedByteSequence(terminal_psi::ByteSequenceCarrier),
    UnresolvedExtent(ScalarApplicationId),
    UnresolvedLayout(StructuralTypeId),
    Overflow,
    MissingCarrierLayout,
    InvalidCarrierLayout,
    AggregateLayout(layout_plans::AggregateGeometryError),
}

impl std::fmt::Display for StorageGeometryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "storage geometry: {self:?}")
    }
}
impl std::error::Error for StorageGeometryError {}

/// One geometry algorithm over borrowed declarations and exact scalar inputs.
/// This does not copy or rewrite the canonical structural catalog. A resolver
/// can report an outstanding extent so its owner evaluates that dependency
/// before attempting geometry again; no guest computation runs in this query.
pub struct ReferenceStorageGeometry<Types, Extents> {
    types: Types,
    extents: Extents,
    carriers: Option<TargetCarrierLayout>,
}

impl<'types, Types, Extents> ReferenceStorageGeometry<Types, Extents>
where
    Types: Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    Extents: FnMut(ScalarApplicationId) -> Result<u64, StorageGeometryError>,
{
    pub fn new(types: Types, extents: Extents) -> Self {
        Self {
            types,
            extents,
            carriers: None,
        }
    }

    /// Layout observations may supply target carrier geometry. Ordinary
    /// storage access queries remain model-free and cannot infer pointer bytes.
    pub fn with_target_carriers(mut self, carriers: Option<TargetCarrierLayout>) -> Self {
        self.carriers = carriers;
        self
    }

    pub fn dependencies(
        &self,
        root: StructuralTypeId,
    ) -> Result<Vec<ScalarApplicationId>, StorageGeometryError> {
        let mut dependencies = Vec::new();
        geometry(
            self.types.clone(),
            root,
            &mut Vec::new(),
            self.carriers,
            &mut |application| {
                if !dependencies.contains(&application) {
                    dependencies.push(application);
                }
                Ok(None)
            },
        )?;
        Ok(dependencies)
    }

    pub fn layout(&mut self, root: StructuralTypeId) -> Result<(u64, u64), StorageGeometryError> {
        geometry(
            self.types.clone(),
            root,
            &mut Vec::new(),
            self.carriers,
            &mut |application| (self.extents)(application).map(Some),
        )?
        .ok_or(StorageGeometryError::UnresolvedLayout(root))
    }
}

/// Discover every extent the ordinary geometry traversal requires before any
/// scalar entry executes. Unknown sizes remain unknown, so later siblings are
/// visited without inventing byte geometry or publishing a placeholder count.
pub fn reference_storage_dependencies<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> Result<Vec<ScalarApplicationId>, StorageGeometryError> {
    ReferenceStorageGeometry::new(types, |application| {
        Err(StorageGeometryError::UnresolvedExtent(application))
    })
    .dependencies(root)
}

fn geometry<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    active: &mut Vec<StructuralTypeId>,
    carriers: Option<TargetCarrierLayout>,
    extents: &mut impl FnMut(ScalarApplicationId) -> Result<Option<u64>, StorageGeometryError>,
) -> Result<Option<(u64, u64)>, StorageGeometryError> {
    if active.len() >= 128 {
        return Err(StorageGeometryError::NestingLimit);
    }
    if active.contains(&root) {
        return Err(StorageGeometryError::RecursiveType(root));
    }
    let shape = &types
        .clone()
        .find(|row| row.id == root)
        .ok_or(StorageGeometryError::MissingType(root))?
        .shape;
    active.push(root);
    let result = (|| match shape {
        // A reference stores its carrier, not the referent. In particular an
        // unavailable or recursive referent layout creates no dependency here.
        StructuralTypeShape::Reference { .. } => {
            let carriers = carriers.ok_or(StorageGeometryError::MissingCarrierLayout)?;
            carrier_geometry(carriers.pointer_byte_size, carriers.pointer_byte_alignment).map(Some)
        }
        StructuralTypeShape::ByteSequence(carrier) => {
            byte_sequence_geometry(*carrier, carriers).map(Some)
        }
        StructuralTypeShape::ElementView { .. } => {
            let carriers = carriers.ok_or(StorageGeometryError::MissingCarrierLayout)?;
            carrier_geometry(
                carriers.descriptor_byte_size,
                carriers.descriptor_byte_alignment,
            )
            .map(Some)
        }
        StructuralTypeShape::PrimitiveScalar(scalar) => scalar_geometry(*scalar)
            .map(Some)
            .ok_or(StorageGeometryError::UnsupportedScalar(*scalar)),
        StructuralTypeShape::Record {
            storage: StructuralRecordStorage::Authored(storage),
            ..
        } => Ok(Some((storage.byte_size, storage.byte_alignment))),
        StructuralTypeShape::Record {
            fields,
            storage: StructuralRecordStorage::Packed,
        } => {
            let mut size = Some(0);
            let mut alignment = 1;
            for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                let child = field_geometry(types.clone(), field, active, carriers, extents)?;
                let Some((child_size, child_alignment)) = child else {
                    size = None;
                    continue;
                };
                if let Some(current) = size {
                    size = Some(
                        align(current, child_alignment)
                            .and_then(|offset| offset.checked_add(child_size))
                            .ok_or(StorageGeometryError::Overflow)?,
                    );
                }
                alignment = alignment.max(child_alignment);
            }
            size.map(|size| {
                Ok((
                    align(size, alignment).ok_or(StorageGeometryError::Overflow)?,
                    alignment,
                ))
            })
            .transpose()
        }
        StructuralTypeShape::FixedArray { element, length } => {
            let length = match length {
                terminal_psi::ArrayExtent::Literal(length) => Some(*length),
                terminal_psi::ArrayExtent::Application(application) => extents(*application)?,
            };
            let element = geometry(types.clone(), *element, active, carriers, extents)?;
            let (Some(length), Some((size, alignment))) = (length, element) else {
                return Ok(None);
            };
            let size = align(size, alignment)
                .and_then(|stride| stride.checked_mul(length))
                .ok_or(StorageGeometryError::Overflow)?;
            Ok(Some((size, alignment)))
        }
        StructuralTypeShape::Sum { cases } => {
            sum_geometry(types, &[], cases, active, carriers, extents)
        }
        StructuralTypeShape::Mixed { fields, cases } => {
            sum_geometry(types, fields, cases, active, carriers, extents)
        }
        _ => Err(StorageGeometryError::UnsupportedType(root)),
    })();
    active.pop();
    result
}

fn sum_geometry<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    common_fields: &[StructuralFieldDeclaration],
    cases: &[terminal_psi::StructuralCaseDeclaration],
    active: &mut Vec<StructuralTypeId>,
    carriers: Option<TargetCarrierLayout>,
    extents: &mut impl FnMut(ScalarApplicationId) -> Result<Option<u64>, StorageGeometryError>,
) -> Result<Option<(u64, u64)>, StorageGeometryError> {
    let mut fields = |fields: &[StructuralFieldDeclaration]| {
        fields
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .map(|field| field_geometry(types.clone(), field, active, carriers, extents))
            .collect::<Result<Vec<_>, _>>()
    };
    let common = fields(common_fields)?;
    let payloads = cases
        .iter()
        .map(|case| fields(&case.fields))
        .collect::<Result<Vec<_>, _>>()?;
    // Discover all dependencies before an unknown extent can defer geometry.
    // Short-circuiting while visiting the fields would hide later case inputs.
    let Some(common) = common.into_iter().collect::<Option<Vec<_>>>() else {
        return Ok(None);
    };
    let Some(payloads) = payloads
        .into_iter()
        .map(|fields| fields.into_iter().collect::<Option<Vec<_>>>())
        .collect::<Option<Vec<_>>>()
    else {
        return Ok(None);
    };
    let carriers = carriers.ok_or(StorageGeometryError::MissingCarrierLayout)?;
    let tag = carrier_geometry(
        carriers.case_tag_byte_size,
        carriers.case_tag_byte_alignment,
    )?;
    let shape = |(byte_size, byte_alignment)| layout_plans::StorageGeometry {
        byte_size,
        byte_alignment,
    };
    let geometry = layout_plans::conventional_sum_geometry(
        shape(tag),
        common.into_iter().map(shape),
        payloads
            .into_iter()
            .map(|fields| fields.into_iter().map(shape)),
    )
    .map_err(StorageGeometryError::AggregateLayout)?;
    Ok(Some((
        geometry.storage.byte_size,
        geometry.storage.byte_alignment,
    )))
}

fn carrier_geometry(size: u64, alignment: u64) -> Result<(u64, u64), StorageGeometryError> {
    if size == 0 || !alignment.is_power_of_two() || size % alignment != 0 {
        return Err(StorageGeometryError::InvalidCarrierLayout);
    }
    Ok((size, alignment))
}

/// A stored byte field and a standalone byte carrier have identical geometry.
/// Fixed views keep their extent in the declaration, not a second runtime word.
fn byte_sequence_geometry(
    carrier: terminal_psi::ByteSequenceCarrier,
    carriers: Option<TargetCarrierLayout>,
) -> Result<(u64, u64), StorageGeometryError> {
    use terminal_psi::ByteSequenceCarrier;
    let carriers = carriers.ok_or(StorageGeometryError::MissingCarrierLayout)?;
    match carrier {
        ByteSequenceCarrier::BorrowedView { .. } => carrier_geometry(
            carriers.descriptor_byte_size,
            carriers.descriptor_byte_alignment,
        ),
        ByteSequenceCarrier::BoundedView { .. } => {
            carrier_geometry(carriers.pointer_byte_size, carriers.pointer_byte_alignment)
        }
        ByteSequenceCarrier::BoundedOwned { .. } => {
            Err(StorageGeometryError::UnsupportedByteSequence(carrier))
        }
    }
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
    ReferenceStorageGeometry::new(types, |application| {
        Err(StorageGeometryError::UnresolvedExtent(application))
    })
    .layout(root)
    .ok()
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
                            let (size, alignment) = field_geometry(
                                types.clone(),
                                candidate,
                                &mut Vec::new(),
                                None,
                                &mut |application| {
                                    Err(StorageGeometryError::UnresolvedExtent(application))
                                },
                            )
                            .ok()??;
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
                StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                },
                StructuralPathSegment::FixedIndex(index),
            ) if index < length => {
                let (size, alignment) = reference_storage_layout(types.clone(), *element)?;
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
    fn extent_discovery_visits_unknown_siblings_but_respects_storage_and_erasure() {
        let application = |value| ScalarApplicationId::new(value).unwrap();
        let declaration = |value, shape| StructuralTypeDeclaration {
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            copyable: true,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id: identity(value),
            identity: terminal_psi::TerminalIdentityIndex(value as u32),
            shape,
        };
        let mut types = vec![
            declaration(1, StructuralTypeShape::PrimitiveScalar(scalar(64))),
            declaration(
                2,
                StructuralTypeShape::FixedArray {
                    element: identity(1),
                    length: terminal_psi::ArrayExtent::Application(application(1)),
                },
            ),
            declaration(
                3,
                StructuralTypeShape::FixedArray {
                    element: identity(1),
                    length: terminal_psi::ArrayExtent::Application(application(2)),
                },
            ),
            declaration(
                4,
                StructuralTypeShape::Record {
                    storage: StructuralRecordStorage::Packed,
                    fields: vec![
                        field(1, StructuralFieldType::Structural(identity(2)), None),
                        field(2, StructuralFieldType::Structural(identity(3)), None),
                    ],
                },
            ),
        ];
        assert_eq!(
            reference_storage_dependencies(types.iter(), identity(4)),
            Ok(vec![application(1), application(2)])
        );
        assert_eq!(
            reference_storage_layout(types.iter(), identity(4)),
            None,
            "discovery cannot publish unknown geometry"
        );
        assert_eq!(
            ReferenceStorageGeometry::new(types.iter(), |dependency| Ok(dependency.get()))
                .layout(identity(4)),
            Ok((24, 8))
        );
        let StructuralTypeShape::Record { fields, .. } = &mut types[3].shape else {
            panic!("record");
        };
        fields[0].relevance = BindingRelevance::Erased;
        assert_eq!(
            reference_storage_dependencies(types.iter(), identity(4)),
            Ok(vec![application(2)])
        );
        let StructuralTypeShape::Record { storage, .. } = &mut types[3].shape else {
            panic!("record");
        };
        *storage = StructuralRecordStorage::Authored(AuthoredRecordStorage {
            byte_size: 3,
            byte_alignment: 1,
        });
        assert_eq!(
            reference_storage_dependencies(types.iter(), identity(4)),
            Ok(Vec::new())
        );
        assert_eq!(
            reference_storage_layout(types.iter(), identity(4)),
            Some((3, 1))
        );

        // Even a zero count does not waive element geometry formation.
        types[1].shape = StructuralTypeShape::FixedArray {
            element: identity(3),
            length: terminal_psi::ArrayExtent::Literal(0),
        };
        assert_eq!(
            reference_storage_dependencies(types.iter(), identity(2)),
            Ok(vec![application(2)])
        );
        types[2].shape = StructuralTypeShape::BindingCarrier;
        assert_eq!(
            reference_storage_dependencies(types.iter(), identity(2)),
            Err(StorageGeometryError::UnsupportedType(identity(3)))
        );
        types[2].shape = StructuralTypeShape::FixedArray {
            element: identity(2),
            length: terminal_psi::ArrayExtent::Literal(0),
        };
        assert_eq!(
            reference_storage_dependencies(types.iter(), identity(2)),
            Err(StorageGeometryError::RecursiveType(identity(2)))
        );
        types[2].shape = StructuralTypeShape::FixedArray {
            element: identity(99),
            length: terminal_psi::ArrayExtent::Literal(0),
        };
        assert_eq!(
            reference_storage_dependencies(types.iter(), identity(2)),
            Err(StorageGeometryError::MissingType(identity(99)))
        );
    }

    #[test]
    fn resolved_array_extent_uses_the_same_geometry_and_reports_missing_or_overflow() {
        let dependency = ScalarApplicationId::new(1).unwrap();
        let mut types = [
            StructuralTypeDeclaration {
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: true,
                copyable: true,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: identity(1),
                identity: terminal_psi::TerminalIdentityIndex(1),
                shape: StructuralTypeShape::PrimitiveScalar(scalar(64)),
            },
            StructuralTypeDeclaration {
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: true,
                copyable: true,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: identity(2),
                identity: terminal_psi::TerminalIdentityIndex(2),
                shape: StructuralTypeShape::FixedArray {
                    element: identity(1),
                    length: terminal_psi::ArrayExtent::Application(dependency),
                },
            },
        ];
        assert_eq!(reference_storage_layout(types.iter(), identity(2)), None);
        assert_eq!(
            ReferenceStorageGeometry::new(types.iter(), |application| Err(
                StorageGeometryError::UnresolvedExtent(application)
            ))
            .layout(identity(2)),
            Err(StorageGeometryError::UnresolvedExtent(dependency))
        );
        assert_eq!(
            ReferenceStorageGeometry::new(types.iter(), |_| Ok(3)).layout(identity(2)),
            Ok((24, 8))
        );
        assert_eq!(
            ReferenceStorageGeometry::new(types.iter(), |_| Ok(0)).layout(identity(2)),
            Ok((0, 8))
        );
        assert_eq!(
            ReferenceStorageGeometry::new(types.iter(), |_| Ok(u64::MAX)).layout(identity(2)),
            Err(StorageGeometryError::Overflow)
        );
        types[1].shape = StructuralTypeShape::FixedArray {
            element: identity(2),
            length: terminal_psi::ArrayExtent::Application(dependency),
        };
        assert_eq!(
            ReferenceStorageGeometry::new(types.iter(), |_| Ok(3)).layout(identity(2)),
            Err(StorageGeometryError::RecursiveType(identity(2)))
        );
    }

    #[test]
    fn canonical_parent_keeps_authored_child_width_and_codec() {
        let types = [
            StructuralTypeDeclaration {
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                copyable: false,
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
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                copyable: false,
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
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            copyable: false,
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
