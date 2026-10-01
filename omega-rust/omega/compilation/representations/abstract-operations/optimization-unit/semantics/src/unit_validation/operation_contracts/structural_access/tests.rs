//! A derived descriptor never acquires the access of an owning result.
use crate::unit_validation::operation_contracts::structural_access::{
    StructuralProjectionPolicy, structural_access_can_supply, structural_arguments_match,
    structural_source_contract,
};
use semantic_vocabulary::{PlaceId, ScalarType, StructuralTypeId};

#[test]
fn copied_byte_field_call_replays_declaration_and_excludes_only_its_consumer() {
    use abstract_operations::AbstractOperation as Operation;
    use semantic_vocabulary::{OperationId, StructuralDomainId, StructuralPlaceKind};
    use terminal_psi::{
        ByteSequenceCarrier, StructuralAccess as Access, StructuralMultiplicity as Multiplicity,
        StructuralPathSegment as Path, StructuralTypeShape as Shape,
    };
    let mut unit =
        crate::tests::fixtures::scalar_units::projected_shared_structural_scalar_call_unit();
    let carrier = unit.structural_types[1].id;
    let domain = StructuralDomainId::new(901).unwrap();
    let copied = PlaceId::new(902).unwrap();
    let producer = OperationId::new(903).unwrap();
    let mut predicates = terminal_psi::ByteSequencePredicateSet::EMPTY;
    predicates.insert(terminal_psi::ByteSequencePredicate::ValidUtf8);
    unit.structural_types.make_mut()[1].shape =
        Shape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 8 });
    let Shape::Record { fields } = &mut unit.structural_types.make_mut()[0].shape else {
        panic!("record");
    };
    fields[0].field_type = terminal_psi::StructuralFieldType::PredicateByteSequence {
        carrier: ByteSequenceCarrier::BoundedOwned { capacity: 8 },
        predicates,
    };
    unit.structural_domains = vec![terminal_psi::StructuralDomainDeclaration {
        id: domain,
        semantic_domain: semantic_vocabulary::DomainSemanticId::new(901).unwrap(),
        identity: "copied-field-Utf8".into(),
        carrier,
        byte_predicate: Some(terminal_psi::ByteSequencePredicate::ValidUtf8),
        content_projection: None,
        establishment_routes: Vec::new(),
    }]
    .into();
    let caller = &mut unit.functions[0];
    let backing = caller.structural_parameters[0].place;
    let mut parameter = caller.structural_parameters[0].clone();
    parameter.structural_type = carrier;
    parameter.access = Access::Owned;
    parameter.qualifications = vec![domain];
    let argument = terminal_psi::StructuralArgument {
        place: copied,
        access: Access::Owned,
        path: Vec::new(),
    };
    let mut copy = caller.blocks[0].nodes[0].clone();
    let consumer = copy.operation.psi_operation().unwrap();
    let Operation::CallStructural {
        structural_arguments,
        ..
    } = &mut caller.blocks[0].nodes[0].operation
    else {
        panic!("call");
    };
    *structural_arguments = vec![argument.clone()];
    copy.operation = Operation::StructuralLeafCopy {
        psi_operation: producer,
        source: backing,
        path: vec![Path::Field("carrier".into())].into(),
        result: terminal_psi::StructuralOperationResult {
            place: copied,
            structural_type: carrier,
            multiplicity: Multiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        },
    };
    caller
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: copied,
            kind: StructuralPlaceKind::OperationResult {
                producer,
                structural_type: carrier,
            },
        });
    caller.blocks[0].nodes.insert(0, copy);
    // Test this call contract directly; complete graph metadata/dominance is
    // independently checked by the unit validator and end-to-end fixture.
    let matches = |unit: &optimization_unit::PsiOptimizationUnit| {
        let types = unit
            .structural_types
            .iter()
            .map(|row| (row.id, row))
            .collect();
        let domains = unit
            .structural_domains
            .iter()
            .map(|row| (row.id, row))
            .collect();
        structural_arguments_match(
            &unit.functions[0],
            Some(consumer),
            std::slice::from_ref(&argument),
            std::slice::from_ref(&parameter),
            &types,
            &domains,
            StructuralProjectionPolicy::Unit,
            false,
        )
    };
    assert!(matches(&unit));
    for corruption in 0..8 {
        let mut changed = unit.clone();
        let caller = &mut changed.functions[0];
        match corruption {
            0 => {
                changed.structural_types.make_mut()[1].shape =
                    Shape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 9 })
            }
            1 => {
                let Shape::Record { fields } = &mut changed.structural_types.make_mut()[0].shape
                else {
                    panic!("record");
                };
                fields[0].field_type = terminal_psi::StructuralFieldType::ByteSequence(
                    ByteSequenceCarrier::BoundedOwned { capacity: 8 },
                );
            }
            2 => caller.structural_parameters[0].access = Access::WriteOnlyBorrow,
            3 => {
                caller
                    .structural_places
                    .iter_mut()
                    .find(|place| place.id == copied)
                    .unwrap()
                    .kind = StructuralPlaceKind::OperationResult {
                    producer: consumer,
                    structural_type: carrier,
                }
            }
            4 => {
                let Operation::StructuralLeafCopy { path, .. } =
                    &mut caller.blocks[0].nodes[0].operation
                else {
                    panic!("copy");
                };
                *path = vec![Path::Referent, Path::Field("carrier".into())].into();
            }
            5 => {
                let mut write = caller.blocks[0].nodes[1].clone();
                let Operation::CallStructural {
                    psi_operation,
                    structural_arguments,
                    ..
                } = &mut write.operation
                else {
                    panic!("call");
                };
                *psi_operation = OperationId::new(904).unwrap();
                structural_arguments[0].access = Access::MutableBorrow;
                caller.blocks[0].nodes.push(write);
            }
            6 => {
                let mut escape = caller.blocks[0].nodes[1].clone();
                escape.operation = Operation::ReturnStructural {
                    psi_edge: semantic_vocabulary::EdgeId::new(905).unwrap(),
                    source: copied,
                    returned_claims: Vec::new(),
                    trivial_affine_locals: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                };
                caller.blocks[0].nodes.push(escape);
            }
            7 => {
                let Operation::CallStructural {
                    structural_arguments,
                    ..
                } = &mut caller.blocks[0].nodes[1].operation
                else {
                    panic!("call");
                };
                structural_arguments[0].access = Access::SharedBorrow;
            }
            _ => unreachable!(),
        }
        assert!(
            !matches(&changed),
            "copied byte call corruption {corruption}"
        );
    }
}

#[test]
fn static_primitive_subloans_reconstruct_interleaved_paths_and_access() {
    use terminal_psi::{
        BindingRelevance, StructuralAccess as Access, StructuralFieldDeclaration,
        StructuralFieldType, StructuralMultiplicity as Multiplicity, StructuralPathSegment as Path,
        StructuralTypeDeclaration, StructuralTypeShape as Shape,
    };
    let mut unit =
        crate::tests::fixtures::scalar_units::projected_shared_structural_scalar_call_unit();
    let primitive = StructuralTypeId::new(9_001).unwrap();
    let array = StructuralTypeId::new(9_002).unwrap();
    let record = StructuralTypeId::new(9_003).unwrap();
    let root = StructuralTypeId::new(9_004).unwrap();
    for (id, shape) in [
        (primitive, Shape::PrimitiveScalar(ScalarType::Boolean)),
        (
            array,
            Shape::FixedArray {
                element: primitive,
                length: 2,
            },
        ),
        (
            record,
            Shape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(9_005).unwrap(),
                    identity: "child".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(array),
                }],
            },
        ),
        (
            root,
            Shape::FixedArray {
                element: record,
                length: 2,
            },
        ),
    ] {
        unit.structural_types
            .make_mut()
            .push(StructuralTypeDeclaration {
                id,
                identity: format!("subloan::{}", id.get()),
                shape,
            });
    }
    let types = unit
        .structural_types
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect();
    let caller = &mut unit.functions[0];
    caller.structural_parameters[0].structural_type = root;
    caller.structural_parameters[0].multiplicity = Multiplicity::Unrestricted;
    caller.structural_parameters[0].qualifications.clear();
    caller.structural_parameters[0]
        .projected_qualifications
        .clear();
    let mut parameter = caller.structural_parameters[0].clone();
    parameter.structural_type = primitive;
    let path = vec![
        Path::FixedIndex(1),
        Path::Field("child".into()),
        Path::FixedIndex(0),
    ];
    let mut argument = terminal_psi::StructuralArgument {
        place: caller.structural_parameters[0].place,
        access: Access::SharedBorrow,
        path: path.clone(),
    };
    for source in [
        Access::SharedBorrow,
        Access::MutableBorrow,
        Access::WriteOnlyBorrow,
    ] {
        caller.structural_parameters[0].access = source;
        for presented in [
            Access::SharedBorrow,
            Access::MutableBorrow,
            Access::WriteOnlyBorrow,
        ] {
            argument.access = presented;
            parameter.access = presented;
            assert_eq!(
                structural_arguments_match(
                    caller,
                    None,
                    std::slice::from_ref(&argument),
                    std::slice::from_ref(&parameter),
                    &types,
                    &Default::default(),
                    StructuralProjectionPolicy::Unit,
                    false
                ),
                structural_access_can_supply(source, presented),
                "{source:?} -> {presented:?}",
            );
        }
    }
    caller.structural_parameters[0].access = Access::MutableBorrow;
    parameter.access = Access::SharedBorrow;
    argument.access = Access::SharedBorrow;
    // Type resolution, rather than same-sized geometry, decides every hop.
    for malformed in [
        vec![
            Path::FixedIndex(2),
            Path::Field("child".into()),
            Path::FixedIndex(0),
        ],
        vec![
            Path::FixedIndex(1),
            Path::Field("other".into()),
            Path::FixedIndex(0),
        ],
        vec![
            Path::FixedIndex(1),
            Path::Field("child".into()),
            Path::FixedIndex(2),
        ],
        vec![Path::FixedIndex(1), Path::Referent, Path::FixedIndex(0)],
    ] {
        argument.path = malformed;
        assert!(!structural_arguments_match(
            caller,
            None,
            std::slice::from_ref(&argument),
            std::slice::from_ref(&parameter),
            &types,
            &Default::default(),
            StructuralProjectionPolicy::Unit,
            false
        ));
    }
    argument.path = path;
    for multiplicity in [Multiplicity::Affine, Multiplicity::Linear] {
        caller.structural_parameters[0].multiplicity = multiplicity;
        assert!(!structural_arguments_match(
            caller,
            None,
            std::slice::from_ref(&argument),
            std::slice::from_ref(&parameter),
            &types,
            &Default::default(),
            StructuralProjectionPolicy::Unit,
            false
        ));
    }
}

#[test]
fn shared_affine_owner_loan_preserves_owner_and_rejects_access_or_place_substitution() {
    use terminal_psi::{StructuralAccess as Access, StructuralMultiplicity as Multiplicity};
    let mut unit =
        crate::tests::fixtures::scalar_units::projected_shared_structural_scalar_call_unit();
    let types = unit
        .structural_types
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect();
    let caller = &mut unit.functions[0];
    caller.structural_parameters[0].access = Access::Owned;
    caller.structural_parameters[0].multiplicity = Multiplicity::Affine;
    let mut parameter = caller.structural_parameters[0].clone();
    parameter.access = Access::SharedBorrow;
    parameter.multiplicity = Multiplicity::Unrestricted;
    let argument = terminal_psi::StructuralArgument {
        place: caller.structural_parameters[0].place,
        access: Access::SharedBorrow,
        path: Vec::new(),
    };
    let accepts = |argument: &terminal_psi::StructuralArgument,
                   parameter: &terminal_psi::StructuralParameterDeclaration| {
        structural_arguments_match(
            caller,
            None,
            std::slice::from_ref(argument),
            std::slice::from_ref(parameter),
            &types,
            &Default::default(),
            StructuralProjectionPolicy::Projected,
            false,
        )
    };
    assert!(accepts(&argument, &parameter));
    assert_eq!(
        structural_source_contract(caller, argument.place, false)
            .unwrap()
            .multiplicity,
        Multiplicity::Affine
    );
    for access in [
        Access::MutableBorrow,
        Access::WriteOnlyBorrow,
        Access::Owned,
    ] {
        let mut changed = argument.clone();
        changed.access = access;
        let mut destination = parameter.clone();
        destination.access = access;
        assert!(!accepts(&changed, &destination));
    }
    let mut changed = argument;
    changed.place = PlaceId::new(999).unwrap();
    assert!(!accepts(&changed, &parameter));
}

#[test]
fn bounded_owner_call_rechecks_predicate_preservation_after_rewriting() {
    use terminal_psi::{
        BindingRelevance, ByteSequenceCarrier, ByteSequencePredicate, ByteSequencePredicateSet,
        StructuralAccess as Access, StructuralFieldDeclaration, StructuralFieldType,
        StructuralMultiplicity as Multiplicity, StructuralPathSegment as Path,
        StructuralTypeDeclaration, StructuralTypeShape as Shape,
    };
    let mut unit =
        crate::tests::fixtures::scalar_units::projected_shared_structural_scalar_call_unit();
    let owner = StructuralTypeId::new(9_020).unwrap();
    let root = StructuralTypeId::new(9_021).unwrap();
    let domain = semantic_vocabulary::StructuralDomainId::new(9_022).unwrap();
    let mut predicates = ByteSequencePredicateSet::EMPTY;
    predicates.insert(ByteSequencePredicate::ValidUtf8);
    let declarations = [
        StructuralTypeDeclaration {
            id: owner,
            identity: "owner".into(),
            shape: Shape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 8 }),
        },
        StructuralTypeDeclaration {
            id: root,
            identity: "container".into(),
            shape: Shape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(9_023).unwrap(),
                    identity: "text".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::PredicateByteSequence {
                        carrier: ByteSequenceCarrier::BoundedOwned { capacity: 8 },
                        predicates,
                    },
                }],
            },
        },
    ];
    let types = declarations.iter().map(|row| (row.id, row)).collect();
    let domain_declaration = terminal_psi::StructuralDomainDeclaration {
        id: domain,
        semantic_domain: semantic_vocabulary::DomainSemanticId::new(9_022).unwrap(),
        identity: "utf8".into(),
        carrier: owner,
        content_projection: None,
        byte_predicate: Some(ByteSequencePredicate::ValidUtf8),
        establishment_routes: Vec::new(),
    };
    let domains = [(domain, &domain_declaration)].into_iter().collect();
    let caller = &mut unit.functions[0];
    caller.entry_claim_declarations = Vec::new();
    caller.content_entry_claims = Vec::new();
    let actual = &mut caller.structural_parameters[0];
    actual.structural_type = root;
    actual.access = Access::MutableBorrow;
    actual.multiplicity = Multiplicity::Unrestricted;
    actual.qualifications.clear();
    actual.projected_qualifications.clear();
    let mut expected = actual.clone();
    expected.structural_type = owner;
    expected.qualifications.push(domain);
    let mut argument = terminal_psi::StructuralArgument {
        place: actual.place,
        path: vec![Path::Field("text".into())],
        access: Access::MutableBorrow,
    };
    let accepts = |argument: &terminal_psi::StructuralArgument,
                   expected: &terminal_psi::StructuralParameterDeclaration| {
        structural_arguments_match(
            caller,
            None,
            std::slice::from_ref(argument),
            std::slice::from_ref(expected),
            &types,
            &domains,
            StructuralProjectionPolicy::Unit,
            false,
        )
    };
    assert!(accepts(&argument, &expected));
    expected.qualifications.clear();
    assert!(
        !accepts(&argument, &expected),
        "rewritten bare exclusive formal cannot erase the caller's UTF-8 invariant"
    );
    expected.access = Access::SharedBorrow;
    argument.access = Access::SharedBorrow;
    assert!(
        accepts(&argument, &expected),
        "shared observation may weaken predicates"
    );
}
