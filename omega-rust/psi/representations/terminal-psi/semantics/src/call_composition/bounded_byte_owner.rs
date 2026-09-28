//! Whole-owner loans preserve capacity, live-length metadata and predicates.
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeShape, TerminalModule,
};

/// Rejoin an ordinary call loan to the original whole bounded owner. An
/// exclusive callee must return every predicate the caller's field requires;
/// passing that field through a bare mutable carrier would lose the invariant.
pub fn bounded_byte_owner_capacity(
    module: &TerminalModule,
    actual: &StructuralParameterDeclaration,
    argument: &StructuralArgument,
    expected: &StructuralParameterDeclaration,
) -> Option<u64> {
    bounded_byte_owner_capacity_from_catalogs(
        &module.structural_types,
        &module.structural_domains,
        actual,
        argument,
        expected,
    )
}

pub fn bounded_byte_owner_capacity_from_catalogs(
    types: &[terminal_psi::StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    actual: &StructuralParameterDeclaration,
    argument: &StructuralArgument,
    expected: &StructuralParameterDeclaration,
) -> Option<u64> {
    if actual.place != argument.place
        || argument.access != expected.access
        || expected.multiplicity != StructuralMultiplicity::Unrestricted
        || !matches!(
            actual.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        )
        || !matches!(
            (actual.access, argument.access),
            (
                StructuralAccess::MutableBorrow,
                StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow
                    | StructuralAccess::SharedBorrow
            ) | (
                StructuralAccess::WriteOnlyBorrow,
                StructuralAccess::WriteOnlyBorrow
            ) | (
                StructuralAccess::SharedBorrow,
                StructuralAccess::SharedBorrow
            )
        )
    {
        return None;
    }
    let expected_type = types
        .iter()
        .find(|row| row.id == expected.structural_type)?;
    let StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity }) =
        expected_type.shape
    else {
        return None;
    };
    let required = crate::byte_parameter_predicates_from_domains(domains, expected)?;
    let (actual_capacity, predicates) = if argument.path.is_empty() {
        let declaration = types.iter().find(|row| row.id == actual.structural_type)?;
        let StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity }) =
            declaration.shape
        else {
            return None;
        };
        (
            capacity,
            crate::byte_parameter_predicates_from_domains(domains, actual)?,
        )
    } else {
        if !actual.qualifications.is_empty()
            || !actual.projected_qualifications.is_empty()
            || !terminal_psi::is_bounded_structural_scalar_store_path(&argument.path)
        {
            return None;
        }
        let (StructuralPathSegment::Field(identity), prefix) = argument.path.split_last()? else {
            return None;
        };
        let declaration =
            crate::runtime_structural_path_tip(types.iter(), actual.structural_type, prefix)?;
        let StructuralTypeShape::Record { fields } = &declaration.shape else {
            return None;
        };
        let field = fields
            .iter()
            .find(|field| field.identity == *identity && !field.relevance.is_erased())?;
        let ByteSequenceCarrier::BoundedOwned { capacity } =
            field.field_type.byte_sequence_carrier()?
        else {
            return None;
        };
        (capacity, field.field_type.byte_predicates())
    };
    if actual_capacity != capacity
        || !predicates.implies(required)
        || (argument.access != StructuralAccess::SharedBorrow && !required.implies(predicates))
    {
        return None;
    }
    Some(capacity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{
        DomainSemanticId, PlaceId, StructuralDomainId, StructuralFieldId, StructuralTypeId,
    };
    use terminal_psi::{
        BindingRelevance, ByteSequencePredicate, ByteSequencePredicateSet,
        StructuralDomainDeclaration, StructuralEstablishmentRoute, StructuralFieldDeclaration,
        StructuralFieldType, StructuralTypeDeclaration,
    };

    fn parameter(place: u64, structural_type: u64) -> StructuralParameterDeclaration {
        StructuralParameterDeclaration {
            place: PlaceId::new(place).unwrap(),
            position: 0,
            is_self: false,
            structural_type: StructuralTypeId::new(structural_type).unwrap(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }
    }

    #[test]
    fn exclusive_owner_loan_cannot_weaken_or_invent_byte_predicates() {
        let owner = StructuralTypeId::new(1).unwrap();
        let root = StructuralTypeId::new(2).unwrap();
        let domain_id = StructuralDomainId::new(1).unwrap();
        let mut utf8 = ByteSequencePredicateSet::EMPTY;
        utf8.insert(ByteSequencePredicate::ValidUtf8);
        let types = [
            StructuralTypeDeclaration {
                id: owner,
                identity: "bytes".into(),
                shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                    capacity: 16,
                }),
            },
            StructuralTypeDeclaration {
                id: root,
                identity: "record".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "text".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::PredicateByteSequence {
                            carrier: ByteSequenceCarrier::BoundedOwned { capacity: 16 },
                            predicates: utf8,
                        },
                    }],
                },
            },
        ];
        let mut domains = [StructuralDomainDeclaration {
            id: domain_id,
            semantic_domain: DomainSemanticId::new(1).unwrap(),
            identity: "predicate".into(),
            carrier: owner,
            content_projection: None,
            byte_predicate: Some(ByteSequencePredicate::ValidUtf8),
            establishment_routes: Vec::new(),
        }];
        let actual = parameter(1, 2);
        let mut expected = parameter(2, 1);
        expected.qualifications.push(domain_id);
        let mut argument = StructuralArgument {
            place: actual.place,
            path: vec![StructuralPathSegment::Field("text".into())],
            access: StructuralAccess::MutableBorrow,
        };
        assert_eq!(
            bounded_byte_owner_capacity_from_catalogs(
                &types, &domains, &actual, &argument, &expected
            ),
            Some(16)
        );
        expected.qualifications.clear();
        assert_eq!(
            bounded_byte_owner_capacity_from_catalogs(
                &types, &domains, &actual, &argument, &expected
            ),
            None,
            "bare exclusive callee may corrupt its qualified caller"
        );
        expected.access = StructuralAccess::SharedBorrow;
        argument.access = StructuralAccess::SharedBorrow;
        assert_eq!(
            bounded_byte_owner_capacity_from_catalogs(
                &types, &domains, &actual, &argument, &expected
            ),
            Some(16),
            "shared observation can weaken"
        );
        expected.qualifications.push(domain_id);
        domains[0].byte_predicate = Some(ByteSequencePredicate::AsciiOnly);
        assert_eq!(
            bounded_byte_owner_capacity_from_catalogs(
                &types, &domains, &actual, &argument, &expected
            ),
            None,
            "UTF-8 does not imply ASCII"
        );
        domains[0].byte_predicate = Some(ByteSequencePredicate::ValidUtf8);
        domains[0]
            .establishment_routes
            .push(StructuralEstablishmentRoute::ExactMachine {
                machine: "issuer".into(),
            });
        assert_eq!(
            bounded_byte_owner_capacity_from_catalogs(
                &types, &domains, &actual, &argument, &expected
            ),
            None,
            "routed membership is not a byte classification"
        );
    }

    #[test]
    fn whole_owner_forwarding_requires_exact_capacity_and_exclusive_restoration() {
        let owner = StructuralTypeId::new(1).unwrap();
        let different = StructuralTypeId::new(2).unwrap();
        let types = [
            StructuralTypeDeclaration {
                id: owner,
                identity: "small".into(),
                shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                    capacity: 8,
                }),
            },
            StructuralTypeDeclaration {
                id: different,
                identity: "large".into(),
                shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                    capacity: 16,
                }),
            },
        ];
        let actual = parameter(1, 1);
        let mut expected = parameter(2, 1);
        let argument = StructuralArgument {
            place: actual.place,
            path: Vec::new(),
            access: StructuralAccess::MutableBorrow,
        };
        assert_eq!(
            bounded_byte_owner_capacity_from_catalogs(&types, &[], &actual, &argument, &expected),
            Some(8)
        );
        expected.structural_type = different;
        assert_eq!(
            bounded_byte_owner_capacity_from_catalogs(&types, &[], &actual, &argument, &expected),
            None
        );
    }
}
