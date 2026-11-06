use super::*;
use semantic_vocabulary::{BlockId, OperationId, StructuralPlaceKind};
use terminal_psi::{
    BoundaryParameterKind, OperationResult, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPathQualification, StructuralPlaceDeclaration,
    StructuralStoreDisposition,
};

fn place(id: u64) -> PlaceId {
    PlaceId::new(id).unwrap()
}

pub(super) fn module(qualified: bool) -> TerminalModule {
    let boolean = StructuralTypeId::new(1).unwrap();
    let shared = StructuralTypeId::new(2).unwrap();
    let mutable_word = StructuralTypeId::new(3).unwrap();
    let domain = StructuralDomainId::new(1).unwrap();
    let mut body = super::unit_machine();
    body.structural_parameters = vec![StructuralParameterDeclaration {
        place: place(1),
        position: 0,
        structural_type: boolean,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: if qualified { vec![domain] } else { Vec::new() },
        projected_qualifications: Vec::new(),
    }];
    body.parameter_order = vec![BoundaryParameterKind::Structural];
    body.structural_places = vec![StructuralPlaceDeclaration {
        id: place(1),
        kind: StructuralPlaceKind::Parameter { position: 0 },
    }];
    let result = |root, carrier, projected_qualifications| {
        OperationResult::Structural(StructuralOperationResult {
            place: place(root),
            structural_type: carrier,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications,
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        })
    };
    let rows = |path| {
        if qualified {
            vec![StructuralPathQualification { path, domain }]
        } else {
            Vec::new()
        }
    };
    body.blocks[0].operations = vec![
        Operation::unsuspended(
            OperationId::new(1).unwrap(),
            result(2, shared, rows(vec![StructuralPathSegment::Referent])),
            OperationKind::EstablishReference {
                source: StructuralArgument::whole(place(1), StructuralAccess::SharedBorrow),
                storage: None,
            },
        ),
        Operation::unsuspended(
            OperationId::new(2).unwrap(),
            result(3, shared, Vec::new()),
            OperationKind::EstablishStructuralLocal {},
        ),
        Operation::unsuspended(
            OperationId::new(3).unwrap(),
            OperationResult::Unit,
            OperationKind::StoreStructuralField {
                disposition: StructuralStoreDisposition::Vacant,
                destination: place(3),
                path: Vec::new(),
                field: None,
                value: StructuralArgument::whole(place(2), StructuralAccess::Owned),
            },
        ),
        Operation::unsuspended(
            OperationId::new(4).unwrap(),
            result(
                4,
                mutable_word,
                rows(vec![
                    StructuralPathSegment::Referent,
                    StructuralPathSegment::Referent,
                ]),
            ),
            OperationKind::EstablishReference {
                source: StructuralArgument::whole(place(3), StructuralAccess::MutableBorrow),
                storage: None,
            },
        ),
    ];
    for operation in &body.blocks[0].operations {
        if let Some(result) = operation.result.structural() {
            body.structural_places.push(StructuralPlaceDeclaration {
                id: result.place,
                kind: StructuralPlaceKind::OperationResult {
                    producer: operation.id,
                    structural_type: result.structural_type,
                },
            });
        }
    }
    let declaration = |id, shape| StructuralTypeDeclaration {
        id,
        identity: TerminalIdentityIndex(id.get() as u32),
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        copyable: id == boolean,
        descriptor_cleanup: id != boolean,
        scalar_restriction: None,
        shape,
    };
    TerminalModule {
        structural_types: vec![
            declaration(
                boolean,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            ),
            declaration(
                shared,
                StructuralTypeShape::Reference {
                    referent: boolean,
                    access: StructuralAccess::SharedBorrow,
                },
            ),
            declaration(
                mutable_word,
                StructuralTypeShape::Reference {
                    referent: shared,
                    access: StructuralAccess::MutableBorrow,
                },
            ),
        ],
        structural_domains: vec![StructuralDomainDeclaration {
            id: domain,
            semantic_domain: DomainSemanticId::new(1).unwrap(),
            identity: TerminalIdentityIndex(4),
            carrier: boolean,
            content_projection: None,
            byte_predicate: None,
            required_memberships: Some(Vec::new()),
            complete_predicate_theory: true,
            predicate: None,
            establishment_routes: Vec::new(),
        }],
        machines: vec![body],
        ..TerminalModule::declarations()
    }
}

#[test]
fn reference_word_store_keeps_its_value_duties_without_a_pointee_address() {
    for qualified in [false, true] {
        let module = module(qualified);
        let body = &module.machines[0];
        let graph = terminal_borrowed_storage_graph(&module);
        let mut analysis =
            DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
        let before_store = (BlockId::new(1).unwrap(), 2);
        let word = DescriptorLocation::whole(place(3));
        assert!(
            analysis
                .storage_origins(body.id, before_store, &word)
                .is_none()
        );
        assert_eq!(
            analysis.payload_targets(
                body.id,
                before_store,
                &DescriptorContentsQuery::from_location(&word)
            ),
            Some(vec![DescriptorPayloadTarget::Storage(word)])
        );
        let formation = &body.blocks[0].operations[3];
        let result = formation.result.structural().unwrap();
        let source = StructuralArgument::whole(place(3), StructuralAccess::MutableBorrow);
        let mut history = PayloadMembershipHistory::for_module_preservation(&module, &graph, body);
        let retained = history
            .reference_formation_memberships(
                &module.identities,
                &mut analysis,
                (BlockId::new(1).unwrap(), 3),
                &source,
                None,
                result,
            )
            .expect(
                "storing a pointer must retain its value's complete roster on the pointer word",
            );
        assert_eq!(
            retained, result.projected_qualifications,
            "qualified={qualified}"
        );
    }
}

fn replaced_word(old_duty: bool, replacement_duty: bool) -> TerminalModule {
    let mut module = module(old_duty);
    let body = &mut module.machines[0];
    // Both scalar homes are owned here, so the old pointer has a known target.
    // That target must not become the destination of the second pointer store.
    body.structural_parameters[0].access = StructuralAccess::Owned;
    let mut replacement = body.structural_parameters[0].clone();
    replacement.place = place(6);
    replacement.position = 1;
    replacement.qualifications = if replacement_duty {
        vec![StructuralDomainId::new(1).unwrap()]
    } else {
        Vec::new()
    };
    body.structural_parameters.push(replacement);
    body.parameter_order.push(BoundaryParameterKind::Structural);
    body.structural_places.push(StructuralPlaceDeclaration {
        id: place(6),
        kind: StructuralPlaceKind::Parameter { position: 1 },
    });

    let mut borrow_word = body.blocks[0].operations.pop().unwrap();
    borrow_word.id = OperationId::new(6).unwrap();
    let mut replacement = body.blocks[0].operations[0].clone();
    replacement.id = OperationId::new(4).unwrap();
    let OperationResult::Structural(result) = &mut replacement.result else {
        unreachable!()
    };
    result.place = place(5);
    result.projected_qualifications = if replacement_duty {
        vec![StructuralPathQualification {
            path: vec![StructuralPathSegment::Referent],
            domain: StructuralDomainId::new(1).unwrap(),
        }]
    } else {
        Vec::new()
    };
    replacement.kind = OperationKind::EstablishReference {
        source: StructuralArgument::whole(place(6), StructuralAccess::SharedBorrow),
        storage: None,
    };
    let OperationResult::Structural(result) = &mut borrow_word.result else {
        unreachable!()
    };
    result.projected_qualifications = if replacement_duty {
        vec![StructuralPathQualification {
            path: vec![
                StructuralPathSegment::Referent,
                StructuralPathSegment::Referent,
            ],
            domain: StructuralDomainId::new(1).unwrap(),
        }]
    } else {
        Vec::new()
    };
    body.blocks[0].operations.extend([
        replacement,
        Operation::unsuspended(
            OperationId::new(5).unwrap(),
            OperationResult::Unit,
            OperationKind::StoreStructuralField {
                disposition: StructuralStoreDisposition::ReplaceDiscardable,
                destination: place(3),
                path: Vec::new(),
                field: None,
                value: StructuralArgument::whole(place(5), StructuralAccess::Owned),
            },
        ),
        borrow_word,
    ]);
    body.structural_places
        .iter_mut()
        .find(|row| row.id == place(4))
        .unwrap()
        .kind = StructuralPlaceKind::OperationResult {
        producer: OperationId::new(6).unwrap(),
        structural_type: StructuralTypeId::new(3).unwrap(),
    };
    body.structural_places.push(StructuralPlaceDeclaration {
        id: place(5),
        kind: StructuralPlaceKind::OperationResult {
            producer: OperationId::new(4).unwrap(),
            structural_type: StructuralTypeId::new(2).unwrap(),
        },
    });
    module
}

#[test]
fn replacing_a_reference_word_uses_the_new_value_roster_not_its_old_pointee() {
    for old_duty in [false, true] {
        for replacement_duty in [false, true] {
            let module = replaced_word(old_duty, replacement_duty);
            let body = &module.machines[0];
            let graph = terminal_borrowed_storage_graph(&module);
            let mut analysis =
                DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
            let word = DescriptorLocation::whole(place(3));
            let block = BlockId::new(1).unwrap();
            assert_eq!(
                analysis.storage_origins(body.id, (block, 4), &word),
                Some(vec![DescriptorLocation::whole(place(1))]),
            );
            assert_eq!(
                analysis.unchanged_payload_store_targets(
                    body.id,
                    (block, 4),
                    &DescriptorLocation::whole(place(5)),
                    &word,
                ),
                Ok((
                    vec![DescriptorPayloadTarget::Storage(word)],
                    StructuralTypeId::new(2).unwrap()
                )),
            );
            let mut history =
                PayloadMembershipHistory::for_module_preservation(&module, &graph, body);
            let current = history
                .current_payload_contract_in(&mut analysis, (block, 5), place(3))
                .unwrap();
            assert_eq!(
                current.projected_qualifications,
                body.blocks[0].operations[3]
                    .result
                    .structural()
                    .unwrap()
                    .projected_qualifications,
                "old={old_duty}, replacement={replacement_duty}",
            );
            let result = body.blocks[0].operations[5].result.structural().unwrap();
            assert_eq!(
                history
                    .reference_formation_memberships(
                        &module.identities,
                        &mut analysis,
                        (block, 5),
                        &StructuralArgument::whole(place(3), StructuralAccess::MutableBorrow),
                        None,
                        result,
                    )
                    .unwrap(),
                result.projected_qualifications,
            );
        }
    }
}

#[test]
fn a_failed_typed_store_query_cannot_fall_back_to_the_old_pointer_target() {
    let module = replaced_word(true, false);
    let body = &module.machines[0];
    let mut graph = terminal_borrowed_storage_graph(&module);
    let block = BlockId::new(1).unwrap();
    // Corrupt only the retained destination certificate. The old pointer's
    // known address cannot excuse disagreement with the actual operation.
    graph[0]
        .payload_stores
        .get_mut(&(block, 4))
        .unwrap()
        .destination_query = DescriptorContentsQuery::from_location(&DescriptorLocation {
        root: place(3),
        path: vec![DescriptorPathSegment::Referent],
    });
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    assert_eq!(
        analysis.storage_origins(body.id, (block, 4), &DescriptorLocation::whole(place(3))),
        Some(vec![DescriptorLocation::whole(place(1))]),
    );
    let mut history = PayloadMembershipHistory::for_module_preservation(&module, &graph, body);
    assert_eq!(
        history
            .current_payload_contract_in(&mut analysis, (block, 5), place(3))
            .unwrap_err(),
        "the original store destination is not a static whole payload",
    );
}
