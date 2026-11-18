//! Share demanded native layouts across the queries in one lowering operation.
//!
//! The semantic catalogs remain unchanged. A prepared input borrows its product
//! and this invocation's scratch, so projection helpers cannot accidentally
//! discard preparation by accepting only a slice of declarations.

use arena::NonDenseIdentities;
use calling_conventions::terminal_layout::NativeGeometry as PhysicalGeometry;
use semantic_vocabulary::{ScalarApplicationId, ScalarType, StructuralTypeId, TargetCarrierLayout};
use target::NativeTarget;
use terminal_psi::{StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration};
use terminal_semantics::{ReferenceStorageRegion, StorageGeometryError};

/// The current physical resolver and the selected target needed by the shared
/// storage-view judgment. A declaration slice alone cannot supply sum tags or
/// distinguish physical reference and sequence carriers from source layouts.
pub struct NativeGeometry<'a> {
    physical: PhysicalGeometry<'a>,
    carriers: TargetCarrierLayout,
}

impl<'a> NativeGeometry<'a> {
    pub fn new(
        declarations: &'a [StructuralTypeDeclaration],
        target: NativeTarget,
    ) -> Result<Self, NonDenseIdentities> {
        Self::with_owned_sequence_bounds(declarations, &[], target)
    }

    pub fn with_owned_sequence_bounds(
        declarations: &'a [StructuralTypeDeclaration],
        bounds: &'a [(StructuralTypeId, u64)],
        target: NativeTarget,
    ) -> Result<Self, NonDenseIdentities> {
        Ok(Self {
            physical: PhysicalGeometry::with_owned_sequence_bounds(declarations, bounds)?,
            carriers: runtime_abi::target_carrier_layout(target),
        })
    }

    fn storage_geometry(
        &self,
    ) -> terminal_semantics::ReferenceStorageGeometry<
        std::slice::Iter<'_, StructuralTypeDeclaration>,
        fn(ScalarApplicationId) -> Result<u64, StorageGeometryError>,
        &Self,
    > {
        terminal_semantics::ReferenceStorageGeometry::new(
            self.iter(),
            unresolved_extent as fn(_) -> _,
        )
        .with_target_carriers(Some(self.carriers))
        .with_record_storage(self)
    }

    pub fn storage_region(
        &self,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<ReferenceStorageRegion> {
        self.storage_geometry().region(root, path)
    }

    pub fn storage_view_source_compatible(
        &self,
        source: StructuralTypeId,
        path: &[StructuralPathSegment],
        reference: StructuralTypeId,
        stored: Option<ScalarType>,
    ) -> bool {
        self.storage_geometry()
            .view_source_compatible(source, path, reference, stored)
    }
}

impl<'a> std::ops::Deref for NativeGeometry<'a> {
    type Target = PhysicalGeometry<'a>;
    fn deref(&self) -> &Self::Target {
        &self.physical
    }
}

impl terminal_semantics::RecordStorageResolver for &NativeGeometry<'_> {
    fn carrier_layout(
        &self,
        owner: StructuralTypeId,
        carrier: &StructuralFieldType,
    ) -> Result<Option<(u64, u64)>, StorageGeometryError> {
        let shape = self.physical.field_shape(carrier, owner).map_err(|error| {
            use calling_conventions::terminal_layout::TerminalLayoutError as Native;
            match error {
                Native::UnknownStructuralType(id) => StorageGeometryError::MissingType(id),
                Native::RecursiveStructuralType(id) => StorageGeometryError::RecursiveType(id),
                Native::StructuralTypeTooLarge(_) => StorageGeometryError::Overflow,
                Native::UnclosedScalarType(scalar) => {
                    StorageGeometryError::UnsupportedScalar(scalar)
                }
                Native::UnclosedArrayExtent(id)
                | Native::UnclosedOwnedSequence(id)
                | Native::UnclosedTargetLayout(id)
                | Native::ReentrantLayoutQuery(id) => StorageGeometryError::UnresolvedLayout(id),
                Native::InvalidStorageGeometry(id) => {
                    StorageGeometryError::InvalidRecordStorage(id)
                }
                Native::UnclosedOpaqueRepresentation(id)
                | Native::UnsupportedStructuralSum(id)
                | Native::EmptyStructuralType(id) => StorageGeometryError::UnsupportedType(id),
            }
        })?;
        Ok(Some((
            u64::from(shape.byte_size),
            u64::from(shape.alignment),
        )))
    }

    fn realization(
        &self,
        record: StructuralTypeId,
        _application: terminal_psi::LayoutApplicationId,
    ) -> Result<&terminal_psi::TargetLayoutRealization, StorageGeometryError> {
        // This owner consumes closed native declarations, not an earlier
        // module's publication. Missing physical layout remains a refusal.
        Err(StorageGeometryError::UnresolvedLayout(record))
    }
}

fn unresolved_extent(application: ScalarApplicationId) -> Result<u64, StorageGeometryError> {
    Err(StorageGeometryError::UnresolvedExtent(application))
}

/// Function contracts normally borrow one shared catalog. Prepare that catalog
/// once, while still accepting separately retained catalogs in received plans.
/// Slice identity is only a scratch-sharing key; layout lookup uses semantic IDs.
pub struct NativeGeometries<'a> {
    catalogs: Vec<NativeGeometry<'a>>,
}

impl<'a> NativeGeometries<'a> {
    pub fn new(
        declarations: impl Iterator<
            Item = (
                &'a [StructuralTypeDeclaration],
                &'a [(semantic_vocabulary::StructuralTypeId, u64)],
            ),
        >,
        target: NativeTarget,
    ) -> Result<Self, NonDenseIdentities> {
        let mut catalogs = declarations.collect::<Vec<_>>();
        catalogs.sort_unstable_by_key(|(rows, bounds)| Self::key(rows, bounds));
        catalogs.dedup_by_key(|(rows, bounds)| Self::key(rows, bounds));
        Ok(Self {
            catalogs: catalogs
                .into_iter()
                .map(|(rows, bounds)| {
                    NativeGeometry::with_owned_sequence_bounds(rows, bounds, target)
                })
                .collect::<Result<_, _>>()?,
        })
    }

    fn key(
        rows: &[StructuralTypeDeclaration],
        bounds: &[(semantic_vocabulary::StructuralTypeId, u64)],
    ) -> (usize, usize, usize, usize) {
        (
            rows.as_ptr() as usize,
            rows.len(),
            bounds.as_ptr() as usize,
            bounds.len(),
        )
    }

    pub fn get(
        &self,
        declarations: &[StructuralTypeDeclaration],
        bounds: &[(semantic_vocabulary::StructuralTypeId, u64)],
    ) -> Option<&NativeGeometry<'a>> {
        let key = Self::key(declarations, bounds);
        let position = self
            .catalogs
            .binary_search_by_key(&key, |geometry| {
                Self::key(geometry, geometry.owned_sequence_bounds())
            })
            .ok()?;
        self.catalogs.get(position)
    }
}

/// An exact integer cast is native when both carriers fit a register shape
/// and the checking rules admit the conversion.
pub fn exact_cast_has_native_carriers(
    source: semantic_vocabulary::IntegerType,
    target: semantic_vocabulary::IntegerType,
) -> bool {
    use semantic_vocabulary::ScalarType;
    calling_conventions::ValueShape::for_scalar(ScalarType::Integer(source)).is_some()
        && calling_conventions::ValueShape::for_scalar(ScalarType::Integer(target)).is_some()
        && source.can_exact_cast_to(target)
}

/// A plan input together with the layouts prepared for its structural
/// declarations. It carries no type membership: every scalar a consumer
/// needs is already closed on the value, parameter or operation it types.
pub struct NativeInput<'a, T> {
    source: &'a T,
    geometry: &'a NativeGeometry<'a>,
}

impl<'a, T> NativeInput<'a, T> {
    pub fn new(source: &'a T, geometry: &'a NativeGeometry<'a>) -> Self {
        Self { source, geometry }
    }

    pub fn geometry(&self) -> &'a NativeGeometry<'a> {
        self.geometry
    }
}

impl<T> std::ops::Deref for NativeInput<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.source
    }
}

pub type NativeFunction<'a> = NativeInput<'a, crate::LegalizedScalarFunction>;

#[cfg(test)]
mod tests {
    use super::{NativeGeometries, NativeGeometry};
    use crate::structural_inputs::structural_reference_input as geometry;
    use calling_conventions::ValueShape;
    use semantic_vocabulary::{
        AuthoredRecordStorage, IntegerSign, IntegerType, ScalarType, StructuralTypeId,
    };
    use terminal_psi::{
        ArrayExtent, StructuralRecordStorage, StructuralTypeDeclaration, StructuralTypeShape,
        TerminalIdentityIndex,
    };

    fn declaration(id: u32, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
        StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            id: StructuralTypeId::new(u64::from(id)).unwrap(),
            identity: TerminalIdentityIndex(id),
            shape,
            requires: Vec::new(),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            scalar_restriction: None,
        }
    }

    #[test]
    fn shared_declarations_keep_distinct_owned_storage_bounds() {
        let element = StructuralTypeId::new(1).unwrap();
        let owned = StructuralTypeId::new(2).unwrap();
        let rows = vec![
            declaration(
                1,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                )),
            ),
            declaration(
                2,
                StructuralTypeShape::ElementView {
                    element,
                    access: None,
                },
            ),
        ];
        let small = [(owned, 1)];
        let large = [(owned, 5)];
        let prepared = NativeGeometries::new(
            [
                (rows.as_slice(), small.as_slice()),
                (rows.as_slice(), large.as_slice()),
                (rows.as_slice(), small.as_slice()),
            ]
            .into_iter(),
            target::NativeTarget::linux_x64(),
        )
        .unwrap();
        assert_eq!(prepared.catalogs.len(), 2);
        assert_eq!(
            prepared.get(&rows, &small).unwrap().shape(owned).unwrap(),
            ValueShape::integer(16, 8)
        );
        assert_eq!(
            prepared.get(&rows, &large).unwrap().shape(owned).unwrap(),
            ValueShape::integer(32, 8)
        );
        assert_eq!(
            prepared
                .get(&rows, &small)
                .unwrap()
                .storage_region(owned, &[])
                .unwrap()
                .byte_length,
            16
        );
        assert_eq!(
            prepared
                .get(&rows, &large)
                .unwrap()
                .storage_region(owned, &[])
                .unwrap()
                .byte_length,
            32
        );
        assert!(prepared.get(&rows, &[]).is_none());
    }

    #[test]
    fn native_geometry_preparation_shares_catalogs_without_eager_layout_rejection() {
        let root = StructuralTypeId::new(1).unwrap();
        let deferred = StructuralTypeId::new(2).unwrap();
        let declarations = [
            declaration(
                1,
                StructuralTypeShape::Record {
                    fields: Vec::new(),
                    storage: StructuralRecordStorage::Authored(AuthoredRecordStorage {
                        byte_size: 32,
                        byte_alignment: 16,
                    }),
                },
            ),
            declaration(
                2,
                StructuralTypeShape::Record {
                    fields: Vec::new(),
                    storage: StructuralRecordStorage::TargetLayout(
                        terminal_psi::LayoutApplicationId(1),
                    ),
                },
            ),
        ];
        let prepared = NativeGeometries::new(
            [
                (declarations.as_slice(), &[][..]),
                (declarations.as_slice(), &[][..]),
            ]
            .into_iter(),
            target::NativeTarget::linux_x64(),
        )
        .unwrap();
        assert_eq!(prepared.catalogs.len(), 1);
        let first = prepared.get(&declarations, &[]).unwrap();
        assert!(std::ptr::eq(
            first,
            prepared.get(&declarations, &[]).unwrap()
        ));
        assert_eq!(
            geometry::shape(root, first),
            Some(ValueShape::integer(32, 16))
        );
        assert_eq!(geometry::shape(deferred, first), None);
        assert_eq!(
            geometry::shape(root, first),
            Some(ValueShape::integer(32, 16))
        );
        assert!(
            NativeGeometry::new(
                &[declarations[0].clone(), declarations[0].clone()],
                target::NativeTarget::linux_x64()
            )
            .is_err()
        );
    }

    #[test]
    fn native_geometry_storage_does_not_grant_scalar_instruction_support() {
        let scalar = StructuralTypeId::new(1).unwrap();
        let array = StructuralTypeId::new(2).unwrap();
        let carrier = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 13).unwrap());
        let declarations = [
            declaration(1, StructuralTypeShape::PrimitiveScalar(carrier)),
            declaration(
                2,
                StructuralTypeShape::FixedArray {
                    element: scalar,
                    length: ArrayExtent::Literal(3),
                },
            ),
        ];
        let prepared =
            NativeGeometry::new(&declarations, target::NativeTarget::linux_x64()).unwrap();
        assert_eq!(
            geometry::shape(scalar, &prepared),
            Some(ValueShape::integer(2, 2))
        );
        assert_eq!(
            geometry::shape(array, &prepared),
            Some(ValueShape::integer(6, 2))
        );
        assert_eq!(geometry::scalar_shape(carrier), None);
        assert_eq!(geometry::primitive_array_shape(array, &prepared), None);
    }

    #[test]
    fn native_geometry_projection_keeps_the_authored_array_footprint() {
        use semantic_vocabulary::{AuthoredFieldStorage, StructuralFieldId};
        use terminal_psi::{
            BindingRelevance, StructuralFieldDeclaration, StructuralFieldType,
            StructuralPathSegment,
        };
        let root = StructuralTypeId::new(1).unwrap();
        let array = StructuralTypeId::new(2).unwrap();
        let element = StructuralTypeId::new(3).unwrap();
        let field = StructuralFieldDeclaration {
            declaration_identity: Default::default(),
            id: StructuralFieldId::new(1).unwrap(),
            identity: TerminalIdentityIndex(4),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(array),
            inline_carrier: None,
            storage: Some(AuthoredFieldStorage::Repeated {
                byte_offset: 8,
                element_stride: 8,
            }),
        };
        let mut declarations = [
            declaration(
                1,
                StructuralTypeShape::Record {
                    fields: vec![field.clone()],
                    storage: StructuralRecordStorage::Authored(AuthoredRecordStorage {
                        byte_size: 32,
                        byte_alignment: 8,
                    }),
                },
            ),
            declaration(
                2,
                StructuralTypeShape::FixedArray {
                    element,
                    length: ArrayExtent::Literal(3),
                },
            ),
            declaration(
                3,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                )),
            ),
        ];
        let path = [
            StructuralPathSegment::Field(field.identity),
            StructuralPathSegment::FixedIndex(2),
        ];
        {
            let prepared =
                NativeGeometry::new(&declarations, target::NativeTarget::linux_x64()).unwrap();
            assert_eq!(
                geometry::field_storage_shape(&field, &prepared, root),
                Some(ValueShape::integer(24, 4))
            );
            assert_eq!(
                geometry::project(root, &path, &prepared),
                Some((element, 24))
            );
        }
        let StructuralTypeShape::Record { fields, .. } = &mut declarations[0].shape else {
            unreachable!()
        };
        fields[0].storage = Some(AuthoredFieldStorage::Repeated {
            byte_offset: 8,
            element_stride: 3,
        });
        let prepared =
            NativeGeometry::new(&declarations, target::NativeTarget::linux_x64()).unwrap();
        assert!(geometry::project(root, &path, &prepared).is_none());
    }

    #[test]
    fn native_descriptor_projection_resolves_an_exact_inline_carrier() {
        use semantic_vocabulary::{AuthoredFieldStorage, StructuralFieldId};
        use terminal_psi::{
            BindingRelevance, ByteSequenceCarrier, StructuralAccess, StructuralFieldDeclaration,
            StructuralFieldType, StructuralPathSegment,
        };

        let root = StructuralTypeId::new(1).unwrap();
        let authored = StructuralTypeId::new(2).unwrap();
        let ownerless = StructuralTypeId::new(3).unwrap();
        let wrong = StructuralTypeId::new(4).unwrap();
        let identity = TerminalIdentityIndex(5);
        let path = [StructuralPathSegment::Field(identity)];
        let borrowed = ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        };
        let mut declarations = [
            declaration(
                1,
                StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        declaration_identity: Default::default(),
                        id: StructuralFieldId::new(1).unwrap(),
                        identity,
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::ByteSequence(borrowed),
                        inline_carrier: Some(authored),
                        storage: Some(AuthoredFieldStorage::Direct { byte_offset: 8 }),
                    }],
                    storage: StructuralRecordStorage::Authored(AuthoredRecordStorage {
                        byte_size: 24,
                        byte_alignment: 8,
                    }),
                },
            ),
            declaration(2, StructuralTypeShape::ByteSequence(borrowed)),
            declaration(
                3,
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: None,
                }),
            ),
            declaration(
                4,
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                    access: Some(StructuralAccess::SharedBorrow),
                    capacity: 5,
                }),
            ),
        ];

        for (inline_carrier, expected) in [
            (Some(authored), Some(authored)),
            (None, Some(ownerless)),
            (Some(wrong), None),
        ] {
            let StructuralTypeShape::Record { fields, .. } = &mut declarations[0].shape else {
                unreachable!()
            };
            fields[0].inline_carrier = inline_carrier;
            let prepared =
                NativeGeometry::new(&declarations, target::NativeTarget::linux_x64()).unwrap();
            let projection = geometry::descriptor_source_geometry(root, &path, &prepared);
            assert_eq!(
                projection.as_ref().map(|geometry| geometry.selected),
                expected
            );
            if let Some(geometry) = projection {
                assert_eq!(geometry.byte_offset, 8);
                assert!(geometry.readable);
            }
        }
    }
}
