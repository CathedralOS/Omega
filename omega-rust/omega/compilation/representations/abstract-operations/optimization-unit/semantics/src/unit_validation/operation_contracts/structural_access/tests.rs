//! A derived descriptor never acquires the access of an owning result.
use crate::unit_validation::operation_contracts::structural_access::{
    StructuralProjectionPolicy, owned_bounded_owner_loan, structural_access_can_supply,
    structural_arguments_match, structural_source_contract,
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
fn shared_literal_loan_replays_content_predicates_without_granting_mutation() {
    use abstract_operations::AbstractOperation as Operation;
    use semantic_vocabulary::{DomainSemanticId, StructuralDomainId};
    use terminal_psi::{ByteSequencePredicate, StructuralAccess as Access};

    let mut unit = crate::tests::fixtures::dominance::byte_literal_boundary_unit();
    let domain = StructuralDomainId::new(9_050).unwrap();
    let mut parameter = unit.boundary_machines[0].structural_parameters[0].clone();
    parameter.qualifications = vec![domain];
    unit.structural_domains = vec![terminal_psi::StructuralDomainDeclaration {
        id: domain,
        semantic_domain: DomainSemanticId::new(9_050).unwrap(),
        identity: "literal-path".into(),
        carrier: parameter.structural_type,
        byte_predicate: Some(ByteSequencePredicate::NoNul),
        content_projection: None,
        establishment_routes: Vec::new(),
    }]
    .into();
    let Operation::EstablishByteSequenceLiteral { place, bytes, .. } =
        &mut unit.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("literal fixture");
    };
    *bytes = b"/adir/x".to_vec();
    let argument = terminal_psi::StructuralArgument {
        place: place.id,
        path: Vec::new(),
        access: Access::SharedBorrow,
    };
    let source = structural_source_contract(&unit.functions[0], argument.place, true)
        .expect("literal descriptor has shared custody");
    assert_eq!(source.access, Access::SharedBorrow);
    assert_eq!(
        source.multiplicity,
        terminal_psi::StructuralMultiplicity::Unrestricted
    );
    let accepts = |unit: &optimization_unit::PsiOptimizationUnit,
                   argument: &terminal_psi::StructuralArgument,
                   parameter: &terminal_psi::StructuralParameterDeclaration| {
        structural_arguments_match(
            &unit.functions[0],
            None,
            std::slice::from_ref(argument),
            std::slice::from_ref(parameter),
            &unit
                .structural_types
                .iter()
                .map(|row| (row.id, row))
                .collect(),
            &unit
                .structural_domains
                .iter()
                .map(|row| (row.id, row))
                .collect(),
            StructuralProjectionPolicy::Unit,
            true,
        )
    };
    assert!(accepts(&unit, &argument, &parameter));
    for access in [
        Access::MutableBorrow,
        Access::WriteOnlyBorrow,
        Access::Owned,
    ] {
        let mut changed_argument = argument.clone();
        changed_argument.access = access;
        let mut changed_parameter = parameter.clone();
        changed_parameter.access = access;
        assert!(!accepts(&unit, &changed_argument, &changed_parameter));
    }
    let mut absent = argument.clone();
    absent.place = PlaceId::new(9_051).unwrap();
    assert!(!accepts(&unit, &absent, &parameter));
    let Operation::EstablishByteSequenceLiteral { bytes, .. } =
        &mut unit.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("literal fixture");
    };
    bytes.push(0);
    assert!(!accepts(&unit, &argument, &parameter));
}

#[test]
fn ordinary_call_owned_byte_result_lends_a_shared_view_without_producer_spelling() {
    use abstract_operations::{AbstractOperation as Operation, AbstractStructuralCallResult};
    use semantic_vocabulary::{
        ClaimId, DomainSemanticId, OperationId, StructuralDomainId, StructuralPlaceKind,
    };
    use terminal_psi::{
        ByteSequenceCarrier, ByteSequencePredicate, StructuralAccess as Access,
        StructuralMultiplicity as Multiplicity, StructuralTypeDeclaration,
        StructuralTypeShape as Shape,
    };

    let mut unit =
        crate::tests::fixtures::scalar_units::projected_shared_structural_scalar_call_unit();
    let owner = StructuralTypeId::new(9_030).unwrap();
    let view = StructuralTypeId::new(9_031).unwrap();
    let domain = StructuralDomainId::new(9_032).unwrap();
    let source = PlaceId::new(9_033).unwrap();
    let producer = OperationId::new(9_034).unwrap();
    unit.structural_types.make_mut().extend([
        StructuralTypeDeclaration {
            id: owner,
            identity: "ordinary-call-owner".into(),
            shape: Shape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 8 }),
        },
        StructuralTypeDeclaration {
            id: view,
            identity: "ordinary-call-view".into(),
            shape: Shape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(Access::SharedBorrow),
            }),
        },
    ]);
    let mut structural_domains = unit.structural_domains.to_vec();
    structural_domains.push(terminal_psi::StructuralDomainDeclaration {
        id: domain,
        semantic_domain: DomainSemanticId::new(9_032).unwrap(),
        identity: "ordinary-call-Utf8".into(),
        carrier: owner,
        content_projection: None,
        byte_predicate: Some(ByteSequencePredicate::ValidUtf8),
        establishment_routes: Vec::new(),
    });
    unit.structural_domains = structural_domains.into();
    let callee = unit.functions[1].machine;
    let caller = &mut unit.functions[0];
    // Exercise this operation contract directly. Cross-machine result
    // correspondence and catalog identity have their own validators; this
    // fixture pins the caller-local completed-result presentation only.
    let mut producer_node = caller.blocks[0].nodes[0].clone();
    producer_node.operation = Operation::CallStructural {
        psi_operation: producer,
        result: AbstractStructuralCallResult::Structural {
            result: terminal_psi::StructuralOperationResult {
                place: source,
                structural_type: owner,
                multiplicity: Multiplicity::Unrestricted,
                qualifications: vec![domain],
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            },
            returned_claim_transfers: Vec::new(),
            selected_evidence: Vec::new(),
        },
        callee,
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        dynamic_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    caller.blocks[0].nodes.insert(0, producer_node);
    caller
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: source,
            kind: StructuralPlaceKind::OperationResult {
                producer,
                structural_type: owner,
            },
        });
    let argument = terminal_psi::StructuralArgument {
        place: source,
        path: Vec::new(),
        access: Access::SharedBorrow,
    };
    let parameter = terminal_psi::StructuralParameterDeclaration {
        place: PlaceId::new(9_035).unwrap(),
        position: 0,
        is_self: false,
        structural_type: view,
        multiplicity: Multiplicity::Unrestricted,
        access: Access::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let accepts = |unit: &optimization_unit::PsiOptimizationUnit,
                   parameter: &terminal_psi::StructuralParameterDeclaration| {
        let types = unit
            .structural_types
            .iter()
            .map(|declaration| (declaration.id, declaration))
            .collect();
        let domains = unit
            .structural_domains
            .iter()
            .map(|declaration| (declaration.id, declaration))
            .collect();
        structural_arguments_match(
            &unit.functions[0],
            None,
            std::slice::from_ref(&argument),
            std::slice::from_ref(parameter),
            &types,
            &domains,
            StructuralProjectionPolicy::Unit,
            false,
        )
    };
    assert!(
        accepts(&unit, &parameter),
        "the view may weaken the qualified owner's predicate roster"
    );
    let mut qualified_owner_parameter = parameter.clone();
    qualified_owner_parameter.structural_type = owner;
    qualified_owner_parameter.qualifications.push(domain);
    assert!(
        accepts(&unit, &qualified_owner_parameter),
        "the exact declared result predicate satisfies an owner formal"
    );

    for corruption in ["claim", "establishment", "projected", "carrier"] {
        let mut changed = unit.clone();
        let result = changed.functions[0]
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.nodes)
            .find_map(|node| match &mut node.operation {
                Operation::CallStructural {
                    psi_operation,
                    result: AbstractStructuralCallResult::Structural { result, .. },
                    ..
                } if *psi_operation == producer => Some(result),
                _ => None,
            })
            .unwrap();
        match corruption {
            "claim" => result
                .claims
                .push(terminal_psi::StructuralResultClaimBinding {
                    claim: ClaimId::new(1).unwrap(),
                    path: Vec::new(),
                }),
            "establishment" => result
                .qualification_establishments
                .push(terminal_psi::ResultQualificationEstablishment { domain, route: 0 }),
            "projected" => {
                result
                    .projected_qualifications
                    .push(terminal_psi::StructuralPathQualification {
                        path: vec![terminal_psi::StructuralPathSegment::FixedIndex(0)],
                        domain,
                    })
            }
            "carrier" => result.structural_type = view,
            _ => unreachable!(),
        }
        if corruption == "carrier" {
            let types = changed
                .structural_types
                .iter()
                .map(|declaration| (declaration.id, declaration))
                .collect();
            let domains = changed
                .structural_domains
                .iter()
                .map(|declaration| (declaration.id, declaration))
                .collect();
            let caller = &changed.functions[0];
            let source_contract = structural_source_contract(caller, source, false).unwrap();
            assert!(
                !owned_bounded_owner_loan(
                    caller,
                    &argument,
                    &qualified_owner_parameter,
                    &source_contract,
                    &types,
                    &domains,
                ),
                "a changed result carrier cannot satisfy the owner's contract"
            );
        } else {
            assert!(
                !accepts(&changed, &parameter),
                "ordinary result corruption: {corruption}"
            );
        }
    }
    let mut affine_owner = unit.clone();
    let result = affine_owner.functions[0]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.nodes)
        .find_map(|node| match &mut node.operation {
            Operation::CallStructural {
                psi_operation,
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            } if *psi_operation == producer => Some(result),
            _ => None,
        })
        .unwrap();
    result.multiplicity = Multiplicity::Affine;
    assert!(
        accepts(&affine_owner, &parameter),
        "a shared view does not transfer an affine owner's custody"
    );
    assert!(
        !accepts(&affine_owner, &qualified_owner_parameter),
        "an affine owner cannot be presented as an unrestricted owner"
    );
    let mut erased_predicate = unit.clone();
    let result = erased_predicate.functions[0]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.nodes)
        .find_map(|node| match &mut node.operation {
            Operation::CallStructural {
                psi_operation,
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            } if *psi_operation == producer => Some(result),
            _ => None,
        })
        .unwrap();
    result.qualifications.clear();
    assert!(
        !accepts(&erased_predicate, &qualified_owner_parameter),
        "a declared result predicate cannot be manufactured after erasure"
    );
}

#[test]
fn shared_byte_view_presentation_follows_exact_entry_and_state_arrivals() {
    use semantic_vocabulary::{BlockId, DomainSemanticId, StructuralDomainId, StructuralPlaceKind};
    use terminal_psi::{
        ByteSequenceCarrier, ByteSequencePredicate, StructuralAccess as Access,
        StructuralMultiplicity as Multiplicity, StructuralTypeDeclaration,
        StructuralTypeShape as Shape,
    };

    let mut unit =
        crate::tests::fixtures::scalar_units::projected_shared_structural_scalar_call_unit();
    let owner = StructuralTypeId::new(9_040).unwrap();
    let view = StructuralTypeId::new(9_041).unwrap();
    let domain = StructuralDomainId::new(9_042).unwrap();
    unit.structural_types.make_mut().extend([
        StructuralTypeDeclaration {
            id: owner,
            identity: "arrival-owner".into(),
            shape: Shape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 8 }),
        },
        StructuralTypeDeclaration {
            id: view,
            identity: "arrival-view".into(),
            shape: Shape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(Access::SharedBorrow),
            }),
        },
    ]);
    let mut structural_domains = unit.structural_domains.to_vec();
    structural_domains.push(terminal_psi::StructuralDomainDeclaration {
        id: domain,
        semantic_domain: DomainSemanticId::new(9_042).unwrap(),
        identity: "arrival-Utf8".into(),
        carrier: owner,
        content_projection: None,
        byte_predicate: Some(ByteSequencePredicate::ValidUtf8),
        establishment_routes: Vec::new(),
    });
    unit.structural_domains = structural_domains.into();

    let caller = &mut unit.functions[0];
    let entry = caller.structural_parameters[0].place;
    caller.structural_parameters[0].structural_type = owner;
    caller.structural_parameters[0].access = Access::MutableBorrow;
    caller.structural_parameters[0].qualifications = vec![domain];
    let mut block = caller.blocks[0].clone();
    block.id = BlockId::new(9_043).unwrap();
    block.nodes.clear();
    block.parameters.clear();
    let state = PlaceId::new(9_044).unwrap();
    let mut state_parameter = caller.structural_parameters[0].clone();
    state_parameter.place = state;
    state_parameter.position = 0;
    state_parameter.is_self = false;
    block.structural_parameters = vec![state_parameter];
    caller.blocks.push(block);
    caller
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: state,
            kind: StructuralPlaceKind::BlockParameter {
                block: BlockId::new(9_043).unwrap(),
                position: 0,
            },
        });
    let parameter = terminal_psi::StructuralParameterDeclaration {
        place: PlaceId::new(9_045).unwrap(),
        position: 0,
        is_self: false,
        structural_type: view,
        multiplicity: Multiplicity::Unrestricted,
        access: Access::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let matches = |unit: &optimization_unit::PsiOptimizationUnit,
                   place: PlaceId,
                   parameter: &terminal_psi::StructuralParameterDeclaration| {
        let types = unit
            .structural_types
            .iter()
            .map(|declaration| (declaration.id, declaration))
            .collect();
        let domains = unit
            .structural_domains
            .iter()
            .map(|declaration| (declaration.id, declaration))
            .collect();
        structural_arguments_match(
            &unit.functions[0],
            None,
            &[terminal_psi::StructuralArgument {
                place,
                path: Vec::new(),
                access: Access::SharedBorrow,
            }],
            std::slice::from_ref(parameter),
            &types,
            &domains,
            StructuralProjectionPolicy::Projected,
            false,
        )
    };
    assert!(matches(&unit, entry, &parameter));
    assert!(matches(&unit, state, &parameter));

    let mut unreadable = unit.clone();
    unreadable.functions[0].structural_parameters[0].access = Access::WriteOnlyBorrow;
    assert!(!matches(&unreadable, entry, &parameter));

    let mut unsupported_predicate = parameter;
    unsupported_predicate.qualifications.push(domain);
    assert!(!matches(&unit, entry, &unsupported_predicate));
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

fn subslice_chain(
    byte_view: bool,
    source_access: terminal_psi::StructuralAccess,
    multiplicities: [terminal_psi::StructuralMultiplicity; 2],
) -> optimization_unit::PsiOptimizationFunction {
    use abstract_operations::AbstractOperation as Operation;
    use semantic_vocabulary::{ObligationId, OperationId, StructuralPlaceKind, ValueId};
    let unit = crate::tests::fixtures::scalar_units::projected_shared_structural_scalar_call_unit();
    let mut function = unit.functions[0].clone();
    let mut parameter = function.structural_parameters[0].clone();
    parameter.access = source_access;
    parameter.qualifications.clear();
    parameter.projected_qualifications.clear();
    let declaration = function
        .structural_places
        .iter()
        .find(|row| row.id == parameter.place)
        .unwrap()
        .clone();
    function.structural_parameters = vec![parameter.clone()];
    function.structural_places = vec![declaration];
    let template = function.blocks[0].nodes[0].clone();
    function.blocks.truncate(1);
    function.blocks[0].structural_parameters.clear();
    function.blocks[0].nodes.clear();
    let mut source = parameter.place;
    for (index, multiplicity) in multiplicities.into_iter().enumerate() {
        let producer = OperationId::new(970 + index as u64).unwrap();
        let result = terminal_psi::StructuralOperationResult {
            place: PlaceId::new(970 + index as u64).unwrap(),
            structural_type: parameter.structural_type,
            multiplicity,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        };
        function
            .structural_places
            .push(terminal_psi::StructuralPlaceDeclaration {
                id: result.place,
                kind: StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type: result.structural_type,
                },
            });
        let mut node = template.clone();
        let start = ValueId::new(981).unwrap();
        let end = ValueId::new(982).unwrap();
        let length = ValueId::new(983).unwrap();
        let obligation = ObligationId::new(984).unwrap();
        node.operation = if byte_view {
            Operation::ByteSequenceSubslice {
                psi_operation: producer,
                result: result.clone(),
                source,
                start,
                end,
                length,
                obligation,
            }
        } else {
            Operation::ElementViewSubslice {
                psi_operation: producer,
                result: result.clone(),
                source,
                start,
                end,
                length,
                obligation,
            }
        };
        source = result.place;
        function.blocks[0].nodes.push(node);
    }
    function
}

#[test]
fn subslice_access_replays_each_source_permission_and_attenuation() {
    use terminal_psi::{StructuralAccess as Access, StructuralMultiplicity as Multiplicity};
    for byte_view in [false, true] {
        for (source, multiplicities, expected) in [
            (
                Access::MutableBorrow,
                [Multiplicity::Affine, Multiplicity::Affine],
                Some(Access::MutableBorrow),
            ),
            (
                Access::MutableBorrow,
                [Multiplicity::Affine, Multiplicity::Unrestricted],
                Some(Access::SharedBorrow),
            ),
            (
                Access::SharedBorrow,
                [Multiplicity::Unrestricted, Multiplicity::Unrestricted],
                Some(Access::SharedBorrow),
            ),
            (
                Access::SharedBorrow,
                [Multiplicity::Affine, Multiplicity::Affine],
                None,
            ),
            (
                Access::MutableBorrow,
                [Multiplicity::Unrestricted, Multiplicity::Affine],
                None,
            ),
            (
                Access::WriteOnlyBorrow,
                [Multiplicity::Unrestricted, Multiplicity::Unrestricted],
                None,
            ),
        ] {
            let function = subslice_chain(byte_view, source, multiplicities);
            let operation = &function.blocks[0].nodes[1].operation;
            assert_eq!(
                super::structural_operation_result_contract(&function, operation)
                    .map(|contract| contract.access),
                expected,
                "byte={byte_view} source={source:?} multiplicities={multiplicities:?}"
            );
        }
    }
}

#[test]
fn subslice_access_rejects_source_identity_type_and_cycle_corruption() {
    use abstract_operations::AbstractOperation as Operation;
    use semantic_vocabulary::{OperationId, StructuralPlaceKind};
    use terminal_psi::{StructuralAccess as Access, StructuralMultiplicity as Multiplicity};
    for byte_view in [false, true] {
        let original = subslice_chain(byte_view, Access::MutableBorrow, [Multiplicity::Affine; 2]);
        for mutation in 0..7 {
            let mut function = original.clone();
            match mutation {
                0 => {
                    function.structural_places[1].kind = StructuralPlaceKind::OperationResult {
                        producer: OperationId::new(999).unwrap(),
                        structural_type: function.structural_parameters[0].structural_type,
                    }
                }
                1 => {
                    function.structural_parameters[0].structural_type =
                        StructuralTypeId::new(999).unwrap()
                }
                2 => function.structural_places[2].id = PlaceId::new(999).unwrap(),
                3 => function.structural_parameters[0].position += 1,
                _ => match &mut function.blocks[0].nodes[0].operation {
                    Operation::ByteSequenceSubslice { source, result, .. }
                    | Operation::ElementViewSubslice { source, result, .. } => match mutation {
                        4 => *source = PlaceId::new(971).unwrap(),
                        5 => result.structural_type = StructuralTypeId::new(999).unwrap(),
                        6 => result.place = PlaceId::new(999).unwrap(),
                        _ => unreachable!(),
                    },
                    _ => unreachable!(),
                },
            }
            assert!(
                super::structural_operation_result_contract(
                    &function,
                    &function.blocks[0].nodes[1].operation
                )
                .is_none(),
                "byte={byte_view} mutation={mutation}"
            );
        }
    }
}

#[test]
fn subslice_access_authenticates_establishment_parent_permission() {
    use abstract_operations::AbstractOperation as Operation;
    use terminal_psi::{StructuralAccess as Access, StructuralMultiplicity as Multiplicity};
    for byte_view in [false, true] {
        let mut function =
            subslice_chain(byte_view, Access::MutableBorrow, [Multiplicity::Affine; 2]);
        let (producer, result) = function.blocks[0].nodes[0]
            .operation
            .structural_result()
            .unwrap();
        function.blocks[0].nodes[0].operation = Operation::EstablishElementView {
            region: terminal_psi::ElementViewRegion::WholeCollection,
            psi_operation: producer,
            result: result.clone(),
            destination: result.place,
            source: terminal_psi::StructuralArgument {
                place: function.structural_parameters[0].place,
                access: Access::MutableBorrow,
                path: Vec::new(),
            },
            element: result.structural_type,
        };
        assert!(
            super::structural_operation_result_contract(
                &function,
                &function.blocks[0].nodes[1].operation
            )
            .is_some()
        );
        function.structural_parameters[0].access = Access::SharedBorrow;
        assert!(
            super::structural_operation_result_contract(
                &function,
                &function.blocks[0].nodes[1].operation
            )
            .is_none()
        );
        function.structural_parameters[0].access = Access::Owned;
        let Operation::EstablishElementView { source, .. } =
            &mut function.blocks[0].nodes[0].operation
        else {
            unreachable!()
        };
        source
            .path
            .push(terminal_psi::StructuralPathSegment::Referent);
        assert!(
            super::structural_operation_result_contract(
                &function,
                &function.blocks[0].nodes[1].operation
            )
            .is_none()
        );
    }
}
