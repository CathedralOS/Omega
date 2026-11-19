/*
We classify borrowed block-parameter joins whose runtime carrier is the
referent's address, and resolve the referent type and access ceiling along
projected paths.

Shared and exclusive joins admit primitive scalars, service binding leaves,
bounded byte owners, and aggregate storage through projections without copying
the referent, whereas canonical byte and element views keep their `{base, extent}`
descriptor carriers. When resolving a loan across reference crossings
(`PathSegment::Referent`), enclosing shared access attenuates descendants to
shared access, and reading a stored pointer through write-only storage is
refused.
*/

use crate::structural_type_catalog::unique_structural_type;
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment as PathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

pub fn is_address_join<'a>(
    parameter: &StructuralParameterDeclaration,
    lookup: impl Fn(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration> + Copy,
) -> bool {
    matches!(
        parameter.access,
        StructuralAccess::SharedBorrow
            | StructuralAccess::MutableBorrow
            | StructuralAccess::WriteOnlyBorrow
    ) && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && lookup(parameter.structural_type).is_some_and(|declaration| {
            declaration.id == parameter.structural_type && !declaration.shape.is_view_descriptor()
        })
}

/// A block edge copies a view's descriptor value, including its retained
/// access. This is distinct from a callable's public ABI presentation.
pub fn is_descriptor_join<'a>(
    parameter: &StructuralParameterDeclaration,
    lookup: impl Fn(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration>,
) -> bool {
    matches!(
        parameter.access,
        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) && matches!(
        parameter.multiplicity,
        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
    ) && lookup(parameter.structural_type).is_some_and(|declaration| {
        declaration.id == parameter.structural_type
            && declaration.shape.is_view_descriptor()
            && (parameter.projected_qualifications.is_empty()
                || matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
                    && parameter
                        .projected_qualifications
                        .iter()
                        .all(|row| row.is_element_membership()))
    })
}

pub fn is_descriptor_join_in(
    parameter: &StructuralParameterDeclaration,
    types: &[StructuralTypeDeclaration],
) -> bool {
    is_descriptor_join(parameter, |identity| {
        unique_structural_type(types.iter(), identity)
    })
}

pub fn is_address_join_in(
    parameter: &StructuralParameterDeclaration,
    types: &[StructuralTypeDeclaration],
) -> bool {
    is_address_join(parameter, |identity| {
        unique_structural_type(types.iter(), identity)
    })
}

pub fn is_static_projection(path: &[PathSegment]) -> bool {
    path.iter().all(|segment| {
        matches!(
            segment,
            PathSegment::Field(_) | PathSegment::Case(_) | PathSegment::FixedIndex(_)
        )
    })
}

pub fn is_address_projection(path: &[PathSegment]) -> bool {
    path.iter().all(|segment| {
        matches!(
            segment,
            PathSegment::Field(_)
                | PathSegment::Case(_)
                | PathSegment::FixedIndex(_)
                | PathSegment::Referent
                | PathSegment::RuntimeIndex { .. }
        )
    })
}

pub fn loan_access_permitted<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[PathSegment],
    source: StructuralAccess,
    destination: StructuralAccess,
) -> bool {
    use StructuralAccess::{MutableBorrow, Owned, SharedBorrow, WriteOnlyBorrow};
    if borrowed_address_access(declarations.clone(), root, path, destination).is_none()
        || (referent_type(declarations.clone(), root, path).is_none()
            && !matches!(
                terminal_semantics::runtime_read_projection_tip(declarations.clone(), root, path),
                Some(terminal_semantics::Position::ByteElement)
            ))
    {
        return false;
    }
    let mut ceiling = source;
    for (position, segment) in path.iter().enumerate() {
        if *segment != PathSegment::Referent {
            continue;
        }
        if ceiling == WriteOnlyBorrow {
            return false;
        }
        let Some(reference) = referent_type(declarations.clone(), root, &path[..position])
            .and_then(|identity| unique_structural_type(declarations.clone(), identity))
        else {
            return false;
        };
        let StructuralTypeShape::Reference { access, .. } = reference.shape else {
            return false;
        };
        ceiling = match ceiling {
            SharedBorrow => SharedBorrow,
            Owned | MutableBorrow => access,
            WriteOnlyBorrow => return false,
        };
    }
    matches!(
        (ceiling, destination),
        (
            Owned | MutableBorrow,
            SharedBorrow | MutableBorrow | WriteOnlyBorrow
        ) | (SharedBorrow, SharedBorrow)
            | (WriteOnlyBorrow, WriteOnlyBorrow)
    )
}

pub fn borrowed_referent_type<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[PathSegment],
    access: StructuralAccess,
    expected: Option<StructuralTypeId>,
) -> Option<StructuralTypeId> {
    borrowed_address_access(declarations.clone(), root, path, access)?;
    terminal_semantics::borrowed_projection_type(declarations, root, path, expected)
}

fn borrowed_address_access<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[PathSegment],
    access: StructuralAccess,
) -> Option<()> {
    if !is_address_projection(path) || access == StructuralAccess::Owned {
        return None;
    }
    for (position, segment) in path.iter().enumerate() {
        if *segment != PathSegment::Referent {
            continue;
        }
        let reference = referent_type(declarations.clone(), root, &path[..position])?;
        let declaration = unique_structural_type(declarations.clone(), reference)?;
        let StructuralTypeShape::Reference {
            access: permission, ..
        } = declaration.shape
        else {
            return None;
        };
        if !(permission == access
            || permission == StructuralAccess::MutableBorrow
                && matches!(
                    access,
                    StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow
                ))
            || permission == StructuralAccess::WriteOnlyBorrow && position + 1 != path.len()
        {
            return None;
        }
    }
    Some(())
}

pub fn referent_type<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[PathSegment],
) -> Option<StructuralTypeId> {
    if !is_address_projection(path) {
        return None;
    }
    terminal_semantics::borrowed_projection_type(declarations, root, path, None)
}

#[cfg(test)]
mod tests {
    use super::borrowed_referent_type;
    use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
    use terminal_psi::{
        BindingRelevance, ByteSequenceCarrier, ByteSequencePredicateSet, StructuralAccess,
        StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
        StructuralTypeDeclaration, StructuralTypeShape,
    };

    const OWNER: ByteSequenceCarrier = ByteSequenceCarrier::BoundedOwned { capacity: 32 };

    fn type_id(raw: u64) -> StructuralTypeId {
        StructuralTypeId::new(raw).unwrap()
    }

    fn idx(name: &str) -> terminal_psi::TerminalIdentityIndex {
        terminal_psi::TerminalIdentityIndex(match name {
            "line" => 1,
            "name" => 2,
            _ => 0,
        })
    }

    fn field(
        raw: u64,
        identity: terminal_psi::TerminalIdentityIndex,
        field_type: StructuralFieldType,
    ) -> StructuralFieldDeclaration {
        StructuralFieldDeclaration {
            declaration_identity: Default::default(),
            inline_carrier: None,
            id: StructuralFieldId::new(raw).unwrap(),
            identity,
            relevance: BindingRelevance::Relevant,
            field_type,
            storage: None,
        }
    }

    fn catalog() -> Vec<StructuralTypeDeclaration> {
        let declaration = |raw, shape| StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            requires: Vec::new(),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            scalar_restriction: None,
            id: type_id(raw),
            identity: terminal_psi::TerminalIdentityIndex(10 + raw as u32),
            shape,
        };
        vec![
            declaration(
                1,
                StructuralTypeShape::Record {
                    fields: vec![
                        field(
                            1,
                            idx("line"),
                            StructuralFieldType::PredicateByteSequence {
                                carrier: OWNER,
                                predicates: ByteSequencePredicateSet::from_bits(1).unwrap(),
                            },
                        ),
                        field(
                            2,
                            idx("name"),
                            StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView {
                                access: Some(StructuralAccess::SharedBorrow),
                            }),
                        ),
                    ],
                    storage: terminal_psi::StructuralRecordStorage::Packed,
                },
            ),
            declaration(2, StructuralTypeShape::ByteSequence(OWNER)),
            declaration(
                3,
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: None,
                }),
            ),
        ]
    }

    #[test]
    fn an_owner_field_loan_names_the_owner_carrier() {
        let types = catalog();
        let line = [StructuralPathSegment::Field(idx("line"))];
        for access in [
            StructuralAccess::MutableBorrow,
            StructuralAccess::SharedBorrow,
        ] {
            assert_eq!(
                borrowed_referent_type(types.iter(), type_id(1), &line, access, None),
                Some(type_id(2))
            );
        }
        assert_eq!(
            borrowed_referent_type(
                types.iter(),
                type_id(1),
                &line,
                StructuralAccess::Owned,
                None
            ),
            None
        );
    }

    #[test]
    fn a_stored_view_field_is_a_descriptor_not_a_referent() {
        let types = catalog();
        let name = [StructuralPathSegment::Field(idx("name"))];
        assert_eq!(
            borrowed_referent_type(
                types.iter(),
                type_id(1),
                &name,
                StructuralAccess::SharedBorrow,
                None
            ),
            None
        );
    }

    #[test]
    fn a_shared_reference_keeps_its_referent_and_access() {
        let mut types = catalog();
        let mut reference = types[0].clone();
        reference.id = type_id(5);
        reference.shape = StructuralTypeShape::Reference {
            access: StructuralAccess::SharedBorrow,
            referent: type_id(1),
        };
        types.push(reference);
        assert_eq!(
            borrowed_referent_type(
                types.iter(),
                type_id(5),
                &[StructuralPathSegment::Referent],
                StructuralAccess::SharedBorrow,
                None,
            ),
            Some(type_id(1))
        );
        let field = [
            StructuralPathSegment::Referent,
            StructuralPathSegment::Field(idx("line")),
        ];
        assert_eq!(
            borrowed_referent_type(
                types.iter(),
                type_id(5),
                &field,
                StructuralAccess::SharedBorrow,
                None
            ),
            Some(type_id(2))
        );
        for access in [
            StructuralAccess::MutableBorrow,
            StructuralAccess::WriteOnlyBorrow,
        ] {
            assert_eq!(
                borrowed_referent_type(types.iter(), type_id(5), &field, access, None),
                None
            );
        }
    }

    #[test]
    fn byte_element_addresses_use_the_existing_scalar_declaration() {
        use semantic_vocabulary::{IntegerSign, IntegerType, ObligationId, ScalarType, ValueId};

        let mut types = catalog();
        let element = [StructuralPathSegment::RuntimeIndex {
            index: ValueId::new(1).unwrap(),
            obligation: ObligationId::new(1).unwrap(),
        }];
        let selected = |types: &[StructuralTypeDeclaration], root, path: &[_]| {
            borrowed_referent_type(
                types.iter(),
                root,
                path,
                StructuralAccess::SharedBorrow,
                None,
            )
        };
        // The byte carrier cannot invent a scalar declaration or select a
        // different integer merely because that declaration is available.
        assert_eq!(selected(&types, type_id(3), &element), None);
        let mut byte = types[2].clone();
        byte.id = type_id(7);
        byte.shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
        ));
        types.push(byte);
        assert_eq!(selected(&types, type_id(3), &element), None);
        types.last_mut().unwrap().shape = StructuralTypeShape::PrimitiveScalar(
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
        );
        assert_eq!(selected(&types, type_id(3), &element), Some(type_id(7)));
        assert!(super::loan_access_permitted(
            types.iter(),
            type_id(3),
            &element,
            StructuralAccess::SharedBorrow,
            StructuralAccess::SharedBorrow,
        ));
        assert!(!super::loan_access_permitted(
            types.iter(),
            type_id(3),
            &element,
            StructuralAccess::SharedBorrow,
            StructuralAccess::MutableBorrow,
        ));
        assert_eq!(
            selected(&types, type_id(2), &[StructuralPathSegment::FixedIndex(31)]),
            Some(type_id(7))
        );
        assert_eq!(
            selected(&types, type_id(2), &[StructuralPathSegment::FixedIndex(32)]),
            None
        );
        assert_eq!(
            selected(
                &types,
                type_id(3),
                &[element[0].clone(), StructuralPathSegment::FixedIndex(0)],
            ),
            None
        );
        let mut duplicate = types.last().unwrap().clone();
        duplicate.id = type_id(8);
        types.push(duplicate);
        assert_eq!(selected(&types, type_id(3), &element), None);
    }

    #[test]
    fn stored_access_views_cross_edges_as_values_but_references_keep_addresses() {
        use super::{is_address_join_in, is_descriptor_join_in};
        use terminal_psi::{StructuralMultiplicity, StructuralParameterDeclaration};
        let mut types = catalog();
        let mut stored = types[2].clone();
        stored.id = type_id(4);
        stored.shape = StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        });
        let mut reference = stored.clone();
        reference.id = type_id(5);
        reference.shape = StructuralTypeShape::Reference {
            referent: stored.id,
            access: StructuralAccess::SharedBorrow,
        };
        types.extend([stored, reference]);
        let mut parameter = StructuralParameterDeclaration {
            place: semantic_vocabulary::PlaceId::new(1).unwrap(),
            position: 0,
            structural_type: type_id(4),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            additional_input_qualifications: Vec::new(),
        };
        assert!(is_descriptor_join_in(&parameter, &types));
        assert!(!is_address_join_in(&parameter, &types));
        for address in [type_id(1), type_id(5)] {
            parameter.structural_type = address;
            assert!(is_address_join_in(&parameter, &types));
            assert!(!is_descriptor_join_in(&parameter, &types));
        }
    }
}
