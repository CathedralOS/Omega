//! Share demanded native layouts across the queries in one lowering operation.
//!
//! The semantic catalogs remain unchanged. A prepared input borrows its product
//! and this invocation's scratch, so projection helpers cannot accidentally
//! discard preparation by accepting only a slice of declarations.

pub use calling_conventions::terminal_layout::NativeGeometry;
use semantic_vocabulary::NonDenseIdentities;
use terminal_psi::StructuralTypeDeclaration;

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
    ) -> Result<Self, NonDenseIdentities> {
        let mut catalogs = declarations.collect::<Vec<_>>();
        catalogs.sort_unstable_by_key(|(rows, bounds)| Self::key(rows, bounds));
        catalogs.dedup_by_key(|(rows, bounds)| Self::key(rows, bounds));
        Ok(Self {
            catalogs: catalogs
                .into_iter()
                .map(|(rows, bounds)| NativeGeometry::with_owned_sequence_bounds(rows, bounds))
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

/// Copy the accepted module's retained integer types into the plan's own
/// membership record. The module must be the exact one origin admission used;
/// clones of the retained owner share that immutable module.
pub fn retained_types(
    closed: &terminal_codec::ClosedTerminalModule,
    psi: terminal_psi::TerminalPsiIdentity,
    module: &terminal_psi::TerminalModule,
) -> Option<crate::RetainedIntegerTypes> {
    (closed.identity() == psi && std::ptr::eq(closed.module(), module)).then(|| {
        crate::RetainedIntegerTypes::from_module(closed.closed_types().retained_integers())
    })
}

/// A plan prepared without an accepted module: unit fixtures and the
/// module-less audit entry.
pub const UNSEALED: &crate::RetainedIntegerTypes = &crate::RetainedIntegerTypes::Unsealed;

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

pub struct NativeInput<'a, T> {
    source: &'a T,
    geometry: &'a NativeGeometry<'a>,
    closed_types: &'a crate::RetainedIntegerTypes,
}

impl<'a, T> NativeInput<'a, T> {
    /// An input with no accepted module behind it: unit fixtures and the
    /// module-less validator entry.
    pub fn new(source: &'a T, geometry: &'a NativeGeometry<'a>) -> Self {
        Self::with_types(source, geometry, UNSEALED)
    }

    /// Published inputs retain their exact catalog. Independent plan metadata
    /// has no publication authority and can only close concrete descriptors.
    pub fn with_types(
        source: &'a T,
        geometry: &'a NativeGeometry<'a>,
        closed_types: &'a crate::RetainedIntegerTypes,
    ) -> Self {
        Self {
            source,
            geometry,
            closed_types,
        }
    }

    pub fn closed_scalar(
        &self,
        scalar: semantic_vocabulary::ScalarType,
    ) -> Option<semantic_vocabulary::ClosedScalarType> {
        self.closed_types.scalar(scalar)
    }

    pub fn closed_integer(
        &self,
        integer: semantic_vocabulary::IntegerType,
    ) -> Option<semantic_vocabulary::ClosedIntegerType> {
        self.closed_types.integer(integer)
    }

    pub const fn closed_types(&self) -> &'a crate::RetainedIntegerTypes {
        self.closed_types
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
            copyable: false,
            descriptor_cleanup: false,
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
        assert!(prepared.get(&rows, &[]).is_none());
    }

    #[test]
    fn published_inputs_refuse_foreign_types_instead_of_closing_them_again() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 13).unwrap();
        let scalar = ScalarType::Integer(integer);
        let geometry = NativeGeometry::new(&[]).unwrap();
        let source = ();
        let independent = super::NativeInput::new(&source, &geometry);
        assert_eq!(independent.closed_integer(integer).unwrap().bits(), 13);
        let mut module = terminal_psi::TerminalModule::declarations();
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let identity = identities.intern("native_closed_input");
        module.identities = identities.build();
        let mut row = declaration(1, StructuralTypeShape::PrimitiveScalar(scalar));
        row.identity = identity;
        row.copyable = true;
        module.structural_types.push(row);
        let published = terminal_codec::ClosedTerminalModule::new(module).unwrap();
        assert!(
            super::retained_types(&published, published.identity(), published.module()).is_some()
        );
        let other =
            terminal_codec::ClosedTerminalModule::new(terminal_psi::TerminalModule::declarations())
                .unwrap();
        assert!(super::retained_types(&published, other.identity(), published.module()).is_none());
        let cloned = published.clone();
        assert!(super::retained_types(&cloned, published.identity(), published.module()).is_some());
        let foreign_owner =
            terminal_codec::ClosedTerminalModule::new(published.module().clone()).unwrap();
        assert_eq!(foreign_owner.identity(), published.identity());
        assert!(
            super::retained_types(&foreign_owner, published.identity(), published.module())
                .is_none()
        );
        let retained =
            super::retained_types(&published, published.identity(), published.module()).unwrap();
        let verified = super::NativeInput::with_types(&source, &geometry, &retained);
        assert_eq!(verified.closed_integer(integer).unwrap().bits(), 13);
        assert!(verified.closed_scalar(scalar).is_some());
        let foreign = IntegerType::new(IntegerSign::Unsigned, 17).unwrap();
        assert!(independent.closed_integer(foreign).is_some());
        assert!(verified.closed_integer(foreign).is_none());
        assert!(
            verified
                .closed_scalar(ScalarType::Integer(foreign))
                .is_none()
        );
        let symbolic = IntegerType::parameterized(
            IntegerSign::Unsigned,
            semantic_vocabulary::WidthParameter {
                owner: semantic_vocabulary::MachineId::new(1).unwrap(),
                value: semantic_vocabulary::ValueId::new(1).unwrap(),
            },
        );
        assert!(independent.closed_integer(symbolic).is_none());
        assert!(verified.closed_integer(symbolic).is_none());
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
        assert!(NativeGeometry::new(&[declarations[0].clone(), declarations[0].clone()]).is_err());
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
        let prepared = NativeGeometry::new(&declarations).unwrap();
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
            let prepared = NativeGeometry::new(&declarations).unwrap();
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
        let prepared = NativeGeometry::new(&declarations).unwrap();
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
            let prepared = NativeGeometry::new(&declarations).unwrap();
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
