//! Share demanded native layouts across the queries in one lowering operation.
//!
//! The semantic catalogs remain unchanged. A prepared input borrows its product
//! and this invocation's scratch, so projection helpers cannot accidentally
//! discard preparation by accepting only a slice of declarations.

pub(crate) use calling_conventions::terminal_layout::NativeGeometry;
use semantic_vocabulary::NonDenseIdentities;
use terminal_psi::StructuralTypeDeclaration;

/// Function contracts normally borrow one shared catalog. Prepare that catalog
/// once, while still accepting separately retained catalogs in received plans.
/// Slice identity is only a scratch-sharing key; layout lookup uses semantic IDs.
pub(crate) struct NativeGeometries<'a> {
    catalogs: Vec<NativeGeometry<'a>>,
}

impl<'a> NativeGeometries<'a> {
    pub(crate) fn new(
        declarations: impl Iterator<Item = &'a [StructuralTypeDeclaration]>,
    ) -> Result<Self, NonDenseIdentities> {
        let mut catalogs = declarations.collect::<Vec<_>>();
        catalogs.sort_unstable_by_key(|rows| Self::key(rows));
        catalogs.dedup_by_key(|rows| Self::key(rows));
        Ok(Self {
            catalogs: catalogs
                .into_iter()
                .map(NativeGeometry::new)
                .collect::<Result<_, _>>()?,
        })
    }

    fn key(rows: &[StructuralTypeDeclaration]) -> (usize, usize) {
        (rows.as_ptr() as usize, rows.len())
    }

    pub(crate) fn get(
        &self,
        declarations: &[StructuralTypeDeclaration],
    ) -> Option<&NativeGeometry<'a>> {
        let key = Self::key(declarations);
        let position = self
            .catalogs
            .binary_search_by_key(&key, |geometry| Self::key(geometry))
            .ok()?;
        self.catalogs.get(position)
    }
}

pub(crate) struct NativeInput<'a, T> {
    source: &'a T,
    geometry: &'a NativeGeometry<'a>,
}

impl<'a, T> NativeInput<'a, T> {
    pub(crate) fn new(source: &'a T, geometry: &'a NativeGeometry<'a>) -> Self {
        Self { source, geometry }
    }

    pub(crate) fn geometry(&self) -> &'a NativeGeometry<'a> {
        self.geometry
    }
}

impl<T> std::ops::Deref for NativeInput<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.source
    }
}

pub(crate) type NativeFunction<'a> = NativeInput<'a, legalized_operations::LegalizedScalarFunction>;
pub(crate) type NativePlan<'a> = NativeInput<'a, abstract_operations::AbstractOperationPlan>;
pub(crate) type NativeUnit<'a> = NativeInput<'a, optimization_unit::PsiOptimizationUnit>;

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
        let prepared =
            NativeGeometries::new([declarations.as_slice(), declarations.as_slice()].into_iter())
                .unwrap();
        assert_eq!(prepared.catalogs.len(), 1);
        let first = prepared.get(&declarations).unwrap();
        assert!(std::ptr::eq(first, prepared.get(&declarations).unwrap()));
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
}
