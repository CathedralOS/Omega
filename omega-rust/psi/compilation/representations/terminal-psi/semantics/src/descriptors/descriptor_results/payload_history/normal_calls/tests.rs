use super::*;
use semantic_vocabulary::{DomainSemanticId, ScalarTerm, ScalarType, StructuralDomainId};
use terminal_psi::{StructuralDomainDeclaration, TerminalIdentityIndex};

mod incoming_stores;
mod primitive_writes;
mod reference_words;
mod selected_lender_geometry;

#[test]
fn normal_return_keeps_complete_original_duties_without_old_scalar_contents() {
    let root = PlaceId::new(1).unwrap();
    let carrier = StructuralTypeId::new(1).unwrap();
    let required = StructuralDomainId::new(1).unwrap();
    let extra = StructuralDomainId::new(2).unwrap();
    let types = [StructuralTypeDeclaration {
        id: carrier,
        identity: TerminalIdentityIndex(1),
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        copyable: true,
        descriptor_cleanup: false,
        scalar_restriction: None,
        shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
    }];
    let domains = [required, extra].map(|id| StructuralDomainDeclaration {
        required_memberships: Some(Vec::new()),
        complete_predicate_theory: true,
        predicate: None,
        id,
        semantic_domain: DomainSemanticId::new(id.get()).unwrap(),
        identity: TerminalIdentityIndex(id.get() as u32 + 1),
        carrier,
        content_projection: None,
        byte_predicate: None,
        establishment_routes: Vec::new(),
    });
    let original = PayloadMembershipContract {
        carrier,
        qualifications: vec![required],
        projected_qualifications: Vec::new(),
        scalar_requirements: Vec::new(),
        scalar_observations: vec![PayloadScalarObservation {
            path: Vec::new(),
            value: Some(ScalarTerm::Boolean(false)),
            entry: None,
        }],
    };
    let before = State {
        values: [(root, original.clone())].into(),
        ..State::default()
    };
    let mut proven = original.clone();
    proven.scalar_observations[0].value = Some(ScalarTerm::Boolean(true));
    let mut stronger = proven.clone();
    stronger.qualifications.push(extra);
    let lost = PayloadMembershipContract {
        qualifications: Vec::new(),
        scalar_observations: Vec::new(),
        ..original.clone()
    };
    for (case, contracts, path, restored) in [
        (
            "every return proves original duty",
            vec![proven.clone(), proven.clone()],
            Vec::new(),
            true,
        ),
        (
            "one return lost duty",
            vec![proven.clone(), lost.clone()],
            Vec::new(),
            false,
        ),
        (
            "one return installed an extra duty",
            vec![proven.clone(), stronger],
            Vec::new(),
            false,
        ),
        ("no completed return", Vec::new(), Vec::new(), false),
        (
            "child proof cannot complete root",
            vec![proven],
            vec![DescriptorPathSegment::Referent],
            false,
        ),
    ] {
        let mut after = State {
            values: [(root, lost.clone())].into(),
            unavailable_preservation: vec![root],
            ..State::default()
        };
        NormalReturnProof {
            location: DescriptorLocation { root, path },
            contracts,
        }
        .restore(&types, &domains, &before, &mut after);
        assert_eq!(
            !after.unavailable_preservation.contains(&root),
            restored,
            "{case}"
        );
        assert!(
            after
                .values
                .get(&root)
                .unwrap()
                .scalar_observations
                .is_empty(),
            "{case}"
        );
        assert_eq!(
            after.values.get(&root).unwrap().qualifications,
            if restored { vec![required] } else { Vec::new() },
            "{case}"
        );
    }
}

pub(super) fn unit_machine() -> TerminalMachine {
    use semantic_vocabulary::{BlockId, ContractId, EdgeId};
    let block = BlockId::new(1).unwrap();
    TerminalMachine {
        assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
        id: MachineId::new(1).unwrap(),
        declaration: None,
        attachment: None,
        nominal_cleanup_subject: None,
        parameters: Vec::new(),
        parameter_order: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: terminal_psi::TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        declared_service_reach: Vec::new(),
        closed_reach_application: None,
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block,
        blocks: vec![terminal_psi::Block::plain(
            block,
            Vec::new(),
            Terminator::ReturnUnit {
                edge: EdgeId::new(1).unwrap(),
                cleanup_actions: Vec::new(),
            },
        )],
        contract: terminal_psi::MachineContract::empty(ContractId::new(1).unwrap()),
    }
}

#[test]
fn normal_runtime_selected_descriptor_free_store_is_not_an_empty_effect() {
    use semantic_vocabulary::{IntegerSign, IntegerType, ObligationId, OperationId, ValueId};
    use terminal_psi::{ArrayExtent, StructuralParameterDeclaration, StructuralPlaceDeclaration};
    let array = PlaceId::new(1).unwrap();
    let value = PlaceId::new(2).unwrap();
    let boolean = StructuralTypeId::new(1).unwrap();
    let array_type = StructuralTypeId::new(2).unwrap();
    let index = ValueId::new(1).unwrap();
    let mut body = unit_machine();
    body.parameters.push(terminal_psi::ValueDeclaration {
        id: index,
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
    });
    body.structural_parameters = [
        (array, array_type, StructuralAccess::MutableBorrow),
        (value, boolean, StructuralAccess::Owned),
    ]
    .into_iter()
    .enumerate()
    .map(
        |(position, (place, structural_type, access))| StructuralParameterDeclaration {
            place,
            position: position as u32,
            structural_type,
            access,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        },
    )
    .collect();
    body.structural_places = body
        .structural_parameters
        .iter()
        .map(|p| StructuralPlaceDeclaration {
            id: p.place,
            kind: StructuralPlaceKind::Parameter {
                position: p.position,
            },
        })
        .collect();
    body.parameter_order = vec![
        terminal_psi::BoundaryParameterKind::Scalar,
        terminal_psi::BoundaryParameterKind::Structural,
        terminal_psi::BoundaryParameterKind::Structural,
    ];
    body.blocks[0].operations.push(Operation::unsuspended(
        OperationId::new(1).unwrap(),
        terminal_psi::OperationResult::Unit,
        OperationKind::StoreStructuralField {
            disposition: terminal_psi::StructuralStoreDisposition::ReplaceDiscardable,
            destination: array,
            path: vec![StructuralPathSegment::RuntimeIndex {
                index,
                obligation: ObligationId::new(1).unwrap(),
            }],
            field: None,
            value: StructuralArgument {
                place: value,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            },
        },
    ));
    let types = [
        (
            boolean,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        ),
        (
            array_type,
            StructuralTypeShape::FixedArray {
                element: boolean,
                length: ArrayExtent::Literal(2),
            },
        ),
    ]
    .into_iter()
    .map(|(id, shape)| StructuralTypeDeclaration {
        id,
        identity: TerminalIdentityIndex(id.get() as u32),
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        copyable: true,
        descriptor_cleanup: false,
        scalar_restriction: None,
        shape,
    })
    .collect();
    let module = TerminalModule {
        structural_types: types,
        machines: vec![body],
        ..TerminalModule::declarations()
    };
    let graph = terminal_borrowed_storage_graph_from(&module, [module.machines[0].id]);
    assert_eq!(graph.len(), 1);
    let position = (module.machines[0].entry, 0);
    assert!(graph[0].flow[&position].derivations.is_empty());
    assert!(graph[0].contents_transfers[&position].is_empty());
    let queries = &graph[0].payload_invalidations[&position];
    assert_eq!(queries.len(), 1);
    let query = queries[0].as_ref().unwrap();
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    let targets = analysis
        .payload_targets(graph[0].id, position, query)
        .unwrap();
    assert_eq!(
        targets,
        vec![
            DescriptorPayloadTarget::Storage(DescriptorLocation {
                root: array,
                path: vec![DescriptorPathSegment::FixedIndex(0)]
            }),
            DescriptorPayloadTarget::Storage(DescriptorLocation {
                root: array,
                path: vec![DescriptorPathSegment::FixedIndex(1)]
            })
        ]
    );
    let mut context = NormalPayloadContext::new(&module, &graph);
    assert!(
        context
            .body_writes(&graph, &mut analysis, graph[0].id)
            .is_err(),
        "an unresolved exact derivation cannot certify no performed write"
    );
}

#[test]
fn normal_nominal_cleanup_cannot_be_omitted_from_write_or_return_evidence() {
    let mut body = unit_machine();
    let Terminator::ReturnUnit {
        cleanup_actions, ..
    } = &mut body.blocks[0].terminator
    else {
        unreachable!()
    };
    cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::InvokeNominal(
        terminal_psi::NominalAffineCleanup {
            retains_descendants: false,
            place: PlaceId::new(1).unwrap(),
            path: Vec::new(),
            structural_type: StructuralTypeId::new(1).unwrap(),
            cleanup_machine: MachineId::new(2).unwrap(),
            consumed_claims: Vec::new(),
            cleanup_receiver: None,
            requirement_obligations: Vec::new(),
        },
    ));
    let module = TerminalModule {
        machines: vec![body],
        ..TerminalModule::declarations()
    };
    let context = NormalPayloadContext::new(&module, &[]);
    assert_eq!(
        context.executable_body(module.machines[0].id).unwrap_err(),
        "the normal payload body has an ordered nominal cleanup effect without a replay association"
    );
}

#[test]
fn normal_provider_fact_stays_on_unique_old_subject_and_needs_prior_membership() {
    use semantic_vocabulary::{
        BoundaryMachineId, ObligationId, OperationId, StructuralFieldId, ValueId,
    };
    use terminal_psi::{
        BindingRelevance, BoundaryCompletion, BoundaryDeclarationRole, BoundaryMachineDeclaration,
        BoundaryMachineResult, BoundaryParameterKind, BoundaryRequirementKind,
        NominalDeclarationIdentity, RecordFieldInitializer, RecordFieldValue,
        StructuralDomainPredicate, StructuralDomainRequirement, StructuralFieldDeclaration,
        StructuralFieldType, StructuralOperationResult, StructuralParameterDeclaration,
        StructuralPlaceDeclaration, StructuralRecordStorage,
    };

    let old = PlaceId::new(1).unwrap();
    let replacement = PlaceId::new(2).unwrap();
    let old_pointer = PlaceId::new(3).unwrap();
    let new_pointer = PlaceId::new(4).unwrap();
    let old_record = PlaceId::new(5).unwrap();
    let new_record = PlaceId::new(6).unwrap();
    let array = PlaceId::new(7).unwrap();
    let boolean = StructuralTypeId::new(1).unwrap();
    let reference = StructuralTypeId::new(2).unwrap();
    let record = StructuralTypeId::new(3).unwrap();
    let array_type = StructuralTypeId::new(4).unwrap();
    let field = StructuralFieldId::new(1).unwrap();
    let field_name = TerminalIdentityIndex(8);
    let domain = StructuralDomainId::new(1).unwrap();
    let boundary = BoundaryMachineId::new(1).unwrap();
    let index = ValueId::new(1).unwrap();
    let source = |place, access| StructuralArgument {
        place,
        access,
        path: Vec::new(),
    };

    for case in ["unique", "ambiguous", "missing prior fact"] {
        let mut body = unit_machine();
        body.parameters.push(terminal_psi::ValueDeclaration {
            id: index,
            scalar_type: ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    64,
                )
                .unwrap(),
            ),
            qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
        });
        body.parameter_order = vec![
            BoundaryParameterKind::Scalar,
            BoundaryParameterKind::Structural,
            BoundaryParameterKind::Structural,
        ];
        body.structural_parameters = [old, replacement]
            .into_iter()
            .enumerate()
            .map(|(position, place)| StructuralParameterDeclaration {
                place,
                position: position as u32,
                structural_type: boolean,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::Owned,
                qualifications: vec![domain],
                projected_qualifications: Vec::new(),
            })
            .collect();
        body.structural_places = body
            .structural_parameters
            .iter()
            .map(|parameter| StructuralPlaceDeclaration {
                id: parameter.place,
                kind: StructuralPlaceKind::Parameter {
                    position: parameter.position,
                },
            })
            .collect();
        let mut establish = |ordinal: u64, place, structural_type, kind| {
            let id = OperationId::new(ordinal).unwrap();
            body.structural_places.push(StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::OperationResult {
                    producer: id,
                    structural_type,
                },
            });
            body.blocks[0].operations.push(Operation::unsuspended(
                id,
                terminal_psi::OperationResult::Structural(StructuralOperationResult {
                    place,
                    structural_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    qualification_establishments: Vec::new(),
                    claims: Vec::new(),
                }),
                kind,
            ));
        };
        establish(
            1,
            old_pointer,
            reference,
            OperationKind::EstablishReference {
                source: source(old, StructuralAccess::MutableBorrow),
                storage: None,
            },
        );
        establish(
            2,
            new_pointer,
            reference,
            OperationKind::EstablishReference {
                source: source(replacement, StructuralAccess::MutableBorrow),
                storage: None,
            },
        );
        for (ordinal, place, pointer) in
            [(3, old_record, old_pointer), (4, new_record, new_pointer)]
        {
            establish(
                ordinal,
                place,
                record,
                OperationKind::EstablishRecord {
                    fields: vec![RecordFieldInitializer {
                        field,
                        value: RecordFieldValue::Structural(source(
                            pointer,
                            StructuralAccess::Owned,
                        )),
                    }],
                },
            );
        }
        establish(
            5,
            array,
            array_type,
            OperationKind::EstablishStructuralArray {
                elements: vec![
                    source(old_record, StructuralAccess::Owned),
                    source(new_record, StructuralAccess::Owned),
                ],
            },
        );
        let argument = StructuralArgument {
            place: array,
            access: StructuralAccess::MutableBorrow,
            path: vec![if case == "ambiguous" {
                StructuralPathSegment::RuntimeIndex {
                    index,
                    obligation: ObligationId::new(1).unwrap(),
                }
            } else {
                StructuralPathSegment::FixedIndex(0)
            }],
        };
        body.blocks[0].operations.push(Operation::unsuspended(
            OperationId::new(6).unwrap(),
            terminal_psi::OperationResult::Unit,
            OperationKind::BoundaryCall {
                boundary,
                arguments: Vec::new(),
                structural_arguments: vec![argument],
                completion_receipts: Vec::new(),
                requirement_obligations: Vec::new(),
            },
        ));
        let types = [
            (
                boolean,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            ),
            (
                reference,
                StructuralTypeShape::Reference {
                    referent: boolean,
                    access: StructuralAccess::MutableBorrow,
                },
            ),
            (
                record,
                StructuralTypeShape::Record {
                    storage: StructuralRecordStorage::Packed,
                    fields: vec![StructuralFieldDeclaration {
                        id: field,
                        identity: field_name,
                        inline_carrier: None,
                        relevance: BindingRelevance::Relevant,
                        storage: None,
                        field_type: StructuralFieldType::Structural(reference),
                    }],
                },
            ),
            (
                array_type,
                StructuralTypeShape::FixedArray {
                    element: record,
                    length: terminal_psi::ArrayExtent::Literal(2),
                },
            ),
        ]
        .into_iter()
        .map(|(id, shape)| StructuralTypeDeclaration {
            id,
            identity: TerminalIdentityIndex(id.get() as u32),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            copyable: id == boolean,
            descriptor_cleanup: id != boolean,
            scalar_restriction: None,
            shape,
        })
        .collect();
        let subject = PlaceId::new(99).unwrap();
        let domains = vec![StructuralDomainDeclaration {
            id: domain,
            semantic_domain: DomainSemanticId::new(1).unwrap(),
            identity: TerminalIdentityIndex(10),
            carrier: boolean,
            required_memberships: Some(Vec::new()),
            complete_predicate_theory: true,
            predicate: Some(StructuralDomainPredicate {
                subject,
                proposition: semantic_vocabulary::Proposition::Equal(
                    ScalarTerm::boolean_field_path(subject, Vec::new()),
                    ScalarTerm::Boolean(true),
                ),
            }),
            content_projection: None,
            byte_predicate: None,
            establishment_routes: Vec::new(),
        }];
        let path = vec![
            StructuralPathSegment::Field(field_name),
            StructuralPathSegment::Referent,
        ];
        let declaration = BoundaryMachineDeclaration {
            id: boundary,
            declaration: None,
            numeric_intrinsic: None,
            role: BoundaryDeclarationRole::Callable,
            kind: BoundaryRequirementKind::Machine,
            completion: BoundaryCompletion::NormalReturn,
            unconditional_termination: false,
            identity: TerminalIdentityIndex(11),
            owner: NominalDeclarationIdentity::new(TerminalIdentityIndex(1)),
            attachment: None,
            parameter_order: vec![BoundaryParameterKind::Structural],
            scalar_parameters: Vec::new(),
            crash_routes: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: PlaceId::new(88).unwrap(),
                position: 0,
                structural_type: record,
                multiplicity: StructuralMultiplicity::Affine,
                access: StructuralAccess::MutableBorrow,
                qualifications: Vec::new(),
                projected_qualifications: vec![terminal_psi::StructuralPathQualification {
                    path: path.clone(),
                    domain,
                }],
            }],
            result: BoundaryMachineResult::Unit,
            result_reference_regions: Vec::new(),
            requires: Vec::new(),
            normal_qualifications: vec![StructuralDomainRequirement {
                argument_index: 0,
                path,
                domain,
            }],
            scalar_domain_requirements: Vec::new(),
            scalar_requires: Vec::new(),
            scalar_ensures: Vec::new(),
            reference_outputs: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            fixed_service_reach: Vec::new(),
            owning_service: None,
            published_service_ceiling: Vec::new(),
        };
        let demand = terminal_psi::BoundaryLenderPreservationDemand {
            machine: body.id,
            operation: OperationId::new(6).unwrap(),
            requirement: declaration.normal_qualifications[0].clone(),
        };
        let module = TerminalModule {
            structural_types: types,
            structural_domains: domains,
            machines: vec![body],
            boundary_machines: vec![declaration],
            boundary_lender_preservation: vec![demand],
            ..TerminalModule::declarations()
        };
        let graph = terminal_borrowed_storage_graph_from(&module, [module.machines[0].id]);
        assert_eq!(graph.len(), 1, "{case}");
        let position = (module.machines[0].entry, 5);
        assert!(
            graph[0].calls.contains_key(&position),
            "normal-only calls retain an invocation"
        );
        let contract = |carrier, qualifications, scalar_observations| PayloadMembershipContract {
            carrier,
            qualifications,
            scalar_observations,
            projected_qualifications: Vec::new(),
            scalar_requirements: Vec::new(),
        };
        let before = State {
            values: [
                (
                    old,
                    contract(
                        boolean,
                        if case == "missing prior fact" {
                            Vec::new()
                        } else {
                            vec![domain]
                        },
                        vec![PayloadScalarObservation {
                            path: Vec::new(),
                            value: Some(ScalarTerm::Boolean(true)),
                            entry: None,
                        }],
                    ),
                ),
                (replacement, contract(boolean, vec![domain], Vec::new())),
            ]
            .into(),
            ..State::default()
        };
        let mut analysis =
            DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
        // Check the physical query itself so the ambiguous case cannot pass
        // merely because a view/array query was unsupported.
        let mut captured_query = graph[0].calls[&position].argument_contents[0]
            .as_ref()
            .unwrap()
            .clone();
        let formal = &module.boundary_machines[0].structural_parameters[0];
        let suffix = descriptor_contents_query(
            &module.structural_types,
            formal.place,
            formal.structural_type,
            &module.boundary_machines[0].normal_qualifications[0].path,
            None,
        )
        .unwrap();
        captured_query.steps.extend(suffix.steps);
        let actual_targets = analysis
            .payload_targets(graph[0].id, position, &captured_query)
            .unwrap();
        let expected_targets = if case == "ambiguous" {
            vec![
                DescriptorPayloadTarget::Storage(DescriptorLocation::whole(old)),
                DescriptorPayloadTarget::Storage(DescriptorLocation::whole(replacement)),
            ]
        } else {
            vec![DescriptorPayloadTarget::Storage(DescriptorLocation::whole(
                old,
            ))]
        };
        assert_eq!(actual_targets, expected_targets, "{case}");
        let context = NormalPayloadContext::new(&module, &graph);
        let facts = context
            .captured_provider_facts(&mut analysis, graph[0].id, position, &before)
            .unwrap_or_else(|reason| {
                assert_ne!(case, "unique", "{reason}");
                assert!(!reason.is_empty());
                Vec::new()
            });
        assert_eq!(facts.len(), usize::from(case == "unique"), "{case}");
        if case == "unique" {
            let mut unrequested = module.clone();
            unrequested.boundary_lender_preservation.clear();
            assert!(
                NormalPayloadContext::new(&unrequested, &graph)
                    .captured_provider_facts(&mut analysis, graph[0].id, position, &before)
                    .unwrap()
                    .is_empty(),
                "declaration rows alone cannot publish an unrequested old-subject fact"
            );
            let provider_id = MachineId::new(2).unwrap();
            let mut provider = unit_machine();
            provider.id = provider_id;
            provider.contract = terminal_psi::MachineContract::empty(
                semantic_vocabulary::ContractId::new(2).unwrap(),
            );
            provider.parameter_order = module.boundary_machines[0].parameter_order.clone();
            provider.structural_parameters =
                module.boundary_machines[0].structural_parameters.clone();
            provider.structural_places = provider
                .structural_parameters
                .iter()
                .map(|formal| StructuralPlaceDeclaration {
                    id: formal.place,
                    kind: StructuralPlaceKind::Parameter {
                        position: formal.position,
                    },
                })
                .collect();
            let mut supplied = module.clone();
            supplied.machines.push(provider);
            let supplied_graph =
                terminal_borrowed_storage_graph_from(&supplied, [graph[0].id, provider_id]);
            let mut supplied_analysis = DescriptorAnalysis::with_borrowed_storage_slots(
                &supplied.structural_types,
                &supplied_graph,
            );
            let mut supplied_context = NormalPayloadContext::new(&supplied, &supplied_graph);
            assert!(
                supplied_context
                    .require_provider_preservation(
                        &mut supplied_analysis,
                        graph[0].id,
                        position,
                        &before,
                        None,
                    )
                    .is_err(),
                "a public normal row does not fulfill its own captured-subject duty"
            );
            supplied_context
                .require_provider_preservation(
                    &mut supplied_analysis,
                    graph[0].id,
                    position,
                    &before,
                    Some(provider_id),
                )
                .expect("a complete nonwriting checked body preserves the original subject");
            require_boundary_lender_preservation(&supplied, boundary, Some(provider_id))
                .expect("the public gate reconstructs the actual pre-call history");

            let provider = &mut supplied.machines[1];
            provider.blocks[0].operations = vec![
                Operation::unsuspended(
                    OperationId::new(1).unwrap(),
                    terminal_psi::OperationResult::Scalar(terminal_psi::ValueDeclaration {
                        id: index,
                        scalar_type: ScalarType::Boolean,
                        qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
                    }),
                    OperationKind::BooleanConstant { value: false },
                ),
                Operation::unsuspended(
                    OperationId::new(2).unwrap(),
                    terminal_psi::OperationResult::Unit,
                    OperationKind::WriteOnlyPrimitiveStore {
                        destination: provider.structural_parameters[0].place,
                        path: module.boundary_machines[0].normal_qualifications[0]
                            .path
                            .clone(),
                        value: index,
                        range_obligation: None,
                    },
                ),
            ];
            let supplied_graph =
                terminal_borrowed_storage_graph_from(&supplied, [graph[0].id, provider_id]);
            let mut supplied_analysis = DescriptorAnalysis::with_borrowed_storage_slots(
                &supplied.structural_types,
                &supplied_graph,
            );
            let mut supplied_context = NormalPayloadContext::new(&supplied, &supplied_graph);
            let writes = supplied_context
                .body_writes(&supplied_graph, &mut supplied_analysis, provider_id)
                .unwrap();
            assert_eq!(writes.len(), 1);
            let mut written = supplied_graph
                .iter()
                .find(|body| body.id == graph[0].id)
                .unwrap()
                .calls[&position]
                .argument_contents[writes[0].parameter]
                .clone()
                .unwrap();
            written.steps.extend_from_slice(&writes[0].steps);
            assert_eq!(
                supplied_analysis.exact_payload_target(graph[0].id, position, &written),
                Some(DescriptorLocation::whole(old)),
                "the real store reaches the captured original subject"
            );
            assert_eq!(
                supplied_context.require_provider_preservation(
                    &mut supplied_analysis,
                    graph[0].id,
                    position,
                    &before,
                    Some(provider_id),
                ),
                Err("the actual scalar write does not prove the complete original predicate")
            );
            supplied.machines[1].blocks[0].operations[0].kind =
                OperationKind::BooleanConstant { value: true };
            let mut supplied_graph =
                terminal_borrowed_storage_graph_from(&supplied, [graph[0].id, provider_id]);
            {
                let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(
                    &supplied.structural_types,
                    &supplied_graph,
                );
                NormalPayloadContext::new(&supplied, &supplied_graph)
                    .require_provider_preservation(
                        &mut analysis,
                        graph[0].id,
                        position,
                        &before,
                        Some(provider_id),
                    )
                    .expect("the actual scalar write proves the original true predicate");
            }
            supplied_graph
                .iter_mut()
                .find(|body| body.id == provider_id)
                .unwrap()
                .primitive_payload_writes
                .clear();
            {
                let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(
                    &supplied.structural_types,
                    &supplied_graph,
                );
                assert_eq!(
                    NormalPayloadContext::new(&supplied, &supplied_graph)
                        .require_provider_preservation(
                            &mut analysis,
                            graph[0].id,
                            position,
                            &before,
                            Some(provider_id)
                        ),
                    Err("the performed writer is not a structural payload store"),
                    "a read footprint is not an authenticated primitive writer"
                );
            }
            let provider = &mut supplied.machines[1];
            provider.blocks[0].operations.extend([
                Operation::unsuspended(
                    OperationId::new(3).unwrap(),
                    terminal_psi::OperationResult::Scalar(terminal_psi::ValueDeclaration {
                        id: ValueId::new(2).unwrap(),
                        scalar_type: ScalarType::Boolean,
                        qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
                    }),
                    OperationKind::BooleanConstant { value: false },
                ),
                Operation::unsuspended(
                    OperationId::new(4).unwrap(),
                    terminal_psi::OperationResult::Unit,
                    OperationKind::WriteOnlyPrimitiveStore {
                        destination: provider.structural_parameters[0].place,
                        path: module.boundary_machines[0].normal_qualifications[0]
                            .path
                            .clone(),
                        value: ValueId::new(2).unwrap(),
                        range_obligation: None,
                    },
                ),
            ]);
            let supplied_graph =
                terminal_borrowed_storage_graph_from(&supplied, [graph[0].id, provider_id]);
            let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(
                &supplied.structural_types,
                &supplied_graph,
            );
            let mut context = NormalPayloadContext::new(&supplied, &supplied_graph);
            assert_eq!(
                context
                    .body_writes(&supplied_graph, &mut analysis, provider_id)
                    .unwrap()
                    .len(),
                2
            );
            assert_eq!(
                context.require_provider_preservation(
                    &mut analysis,
                    graph[0].id,
                    position,
                    &before,
                    Some(provider_id)
                ),
                Err("the actual scalar write does not prove the complete original predicate"),
                "one good writer cannot hide another writer to the same original subject"
            );
        }
        let mut after = State {
            values: [
                (old, contract(boolean, Vec::new(), Vec::new())),
                (replacement, contract(boolean, Vec::new(), Vec::new())),
                (array, contract(array_type, Vec::new(), Vec::new())),
            ]
            .into(),
            unavailable_preservation: vec![old],
            ..State::default()
        };
        for fact in facts {
            fact.publish(&module.structural_types, &mut after);
        }
        assert_eq!(
            after.values.get(&old).unwrap().qualifications,
            if case == "unique" {
                vec![domain]
            } else {
                Vec::new()
            },
            "{case}"
        );
        assert!(
            after
                .values
                .get(&replacement)
                .unwrap()
                .qualifications
                .is_empty(),
            "{case}"
        );
        assert!(
            after
                .values
                .get(&array)
                .unwrap()
                .projected_qualifications
                .is_empty(),
            "{case}"
        );
        assert!(
            after
                .values
                .get(&old)
                .unwrap()
                .scalar_observations
                .is_empty(),
            "{case}"
        );
        assert_eq!(after.unavailable_preservation, vec![old], "{case}");
    }
}

#[test]
fn normal_borrowed_scalar_arrival_retains_every_owned_record_source() {
    use semantic_vocabulary::{BlockId, EdgeId, OperationId, StructuralFieldId, ValueId};
    use terminal_psi::{
        BindingRelevance, BoundaryParameterKind, StructuralFieldDeclaration, StructuralFieldType,
        StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralRecordStorage,
        SuccessorEdge,
    };

    let boolean = StructuralTypeId::new(1).unwrap();
    let reference = StructuralTypeId::new(2).unwrap();
    let record = StructuralTypeId::new(3).unwrap();
    let left = PlaceId::new(1).unwrap();
    let right = PlaceId::new(2).unwrap();
    let value = PlaceId::new(3).unwrap();
    let borrowed = PlaceId::new(4).unwrap();
    let field = StructuralFieldId::new(1).unwrap();
    let field_name = TerminalIdentityIndex(8);
    let choice = ValueId::new(1).unwrap();
    let receiving_block = BlockId::new(2).unwrap();
    let suffix = vec![
        DescriptorContentsStep::Address(DescriptorPathSegment::Field(field)),
        DescriptorContentsStep::Address(DescriptorPathSegment::Referent),
    ];

    for distinct_sources in [false, true] {
        let mut body = unit_machine();
        body.parameters.push(terminal_psi::ValueDeclaration {
            id: choice,
            scalar_type: ScalarType::Boolean,
            qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
        });
        body.structural_parameters = [(left, record), (right, record), (value, boolean)]
            .into_iter()
            .enumerate()
            .map(
                |(position, (place, structural_type))| StructuralParameterDeclaration {
                    place,
                    position: position as u32,
                    structural_type,
                    multiplicity: if structural_type == boolean {
                        StructuralMultiplicity::Unrestricted
                    } else {
                        StructuralMultiplicity::Affine
                    },
                    access: StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                },
            )
            .collect();
        body.parameter_order = vec![
            BoundaryParameterKind::Scalar,
            BoundaryParameterKind::Structural,
            BoundaryParameterKind::Structural,
            BoundaryParameterKind::Structural,
        ];
        body.structural_places = body
            .structural_parameters
            .iter()
            .map(|parameter| StructuralPlaceDeclaration {
                id: parameter.place,
                kind: StructuralPlaceKind::Parameter {
                    position: parameter.position,
                },
            })
            .collect();
        body.structural_places.push(StructuralPlaceDeclaration {
            id: borrowed,
            kind: StructuralPlaceKind::BlockParameter {
                block: receiving_block,
                position: 0,
            },
        });
        let edge = |ordinal, source| SuccessorEdge {
            edge: EdgeId::new(ordinal).unwrap(),
            target: receiving_block,
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: source,
                path: vec![
                    StructuralPathSegment::Field(field_name),
                    StructuralPathSegment::Referent,
                ],
                access: StructuralAccess::MutableBorrow,
            }],
            cleanup_actions: Vec::new(),
        };
        body.blocks[0].terminator = Terminator::Conditional {
            condition: choice,
            when_true: edge(1, left),
            when_false: edge(2, if distinct_sources { right } else { left }),
        };
        let mut receiving = terminal_psi::Block::plain(
            receiving_block,
            Vec::new(),
            Terminator::ReturnUnit {
                edge: EdgeId::new(3).unwrap(),
                cleanup_actions: Vec::new(),
            },
        );
        receiving.operations.push(Operation::unsuspended(
            OperationId::new(1).unwrap(),
            terminal_psi::OperationResult::Unit,
            OperationKind::StoreStructuralField {
                disposition: terminal_psi::StructuralStoreDisposition::ReplaceDiscardable,
                destination: borrowed,
                path: Vec::new(),
                field: None,
                value: StructuralArgument {
                    place: value,
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                },
            },
        ));
        receiving
            .structural_parameters
            .push(StructuralParameterDeclaration {
                place: borrowed,
                position: 0,
                structural_type: boolean,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::MutableBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            });
        body.blocks.push(receiving);
        let types = [
            (
                boolean,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            ),
            (
                reference,
                StructuralTypeShape::Reference {
                    referent: boolean,
                    access: StructuralAccess::MutableBorrow,
                },
            ),
            (
                record,
                StructuralTypeShape::Record {
                    storage: StructuralRecordStorage::Packed,
                    fields: vec![StructuralFieldDeclaration {
                        id: field,
                        identity: field_name,
                        inline_carrier: None,
                        relevance: BindingRelevance::Relevant,
                        storage: None,
                        field_type: StructuralFieldType::Structural(reference),
                    }],
                },
            ),
        ]
        .into_iter()
        .map(|(id, shape)| StructuralTypeDeclaration {
            id,
            identity: TerminalIdentityIndex(id.get() as u32),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            copyable: id == boolean,
            descriptor_cleanup: id != boolean,
            scalar_restriction: None,
            shape,
        })
        .collect();
        let module = TerminalModule {
            structural_types: types,
            machines: vec![body],
            ..TerminalModule::declarations()
        };
        let graph = terminal_borrowed_storage_graph_from(&module, [module.machines[0].id]);
        assert_eq!(graph.len(), 1, "real typed Terminal graph must adapt");
        let mut analysis =
            DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
        let selected = analysis
            .payload_targets(
                graph[0].id,
                (receiving_block, 0),
                &DescriptorContentsQuery::from_location(&DescriptorLocation::whole(borrowed)),
            )
            .unwrap();
        let expected_sources = if distinct_sources {
            vec![left, right]
        } else {
            vec![left]
        };
        let expected_targets = expected_sources
            .iter()
            .map(|root| {
                DescriptorPayloadTarget::Incoming(DescriptorContentsQuery {
                    root: *root,
                    steps: suffix.clone(),
                })
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            selected.into_iter().collect::<BTreeSet<_>>(),
            expected_targets,
            "all feasible captured addresses remain distinct from payload values"
        );
        let mut context = NormalPayloadContext::new(&module, &graph);
        let effects = context
            .body_writes(&graph, &mut analysis, graph[0].id)
            .expect("borrowed arrival writes must map back to original machine inputs");
        let actual = effects
            .into_iter()
            .map(|effect| (effect.parameter, effect.steps))
            .collect::<BTreeSet<_>>();
        let expected = expected_sources
            .iter()
            .map(|root| (if *root == left { 0 } else { 1 }, suffix.clone()))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actual, expected,
            "write summary includes every feasible original input and no synthetic arrival home"
        );
    }
}

#[test]
fn normal_owned_record_boolean_store_conserves_old_duty_but_not_old_scalar_theory() {
    use semantic_vocabulary::{ContractId, OperationId, Proposition, StructuralFieldId};
    use terminal_psi::{
        BindingRelevance, BoundaryParameterKind, RecordFieldInitializer, RecordFieldValue,
        StructuralDomainPredicate, StructuralFieldDeclaration, StructuralFieldType,
        StructuralOperationResult, StructuralParameterDeclaration, StructuralPathQualification,
        StructuralPlaceDeclaration, StructuralRecordStorage, StructuralStoreDisposition,
    };

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Writers {
        PromisedReplacement,
        AdditionalUnknownReplacement,
        AdditionalDeeperUnknownReplacement,
    }

    let boolean = StructuralTypeId::new(1).unwrap();
    let reference = StructuralTypeId::new(2).unwrap();
    let record = StructuralTypeId::new(3).unwrap();
    let domain = StructuralDomainId::new(1).unwrap();
    let field = StructuralFieldId::new(1).unwrap();
    let field_name = TerminalIdentityIndex(8);
    let caller_id = MachineId::new(1).unwrap();
    let writer_id = MachineId::new(2).unwrap();
    let deeper_id = MachineId::new(3).unwrap();
    let old = PlaceId::new(1).unwrap();
    let replacement = PlaceId::new(2).unwrap();
    let unknown = PlaceId::new(3).unwrap();
    let pointer = PlaceId::new(4).unwrap();
    let captured = PlaceId::new(5).unwrap();
    let formal_record = PlaceId::new(11).unwrap();
    let formal_replacement = PlaceId::new(12).unwrap();
    let formal_unknown = PlaceId::new(13).unwrap();
    let deeper_destination = PlaceId::new(21).unwrap();
    let deeper_unknown = PlaceId::new(22).unwrap();
    let selected_path = vec![
        StructuralPathSegment::Field(field_name),
        StructuralPathSegment::Referent,
    ];
    let descriptor_selected_path = vec![
        DescriptorPathSegment::Field(field),
        DescriptorPathSegment::Referent,
    ];
    let argument = |place, access, path| StructuralArgument {
        place,
        access,
        path,
    };
    let call = |callee, structural_arguments| OperationKind::CallUnit {
        callee,
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments,
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    let parameters =
        |body: &mut TerminalMachine,
         rows: Vec<(PlaceId, StructuralTypeId, StructuralAccess, bool)>| {
            for (position, (place, structural_type, access, promises_domain)) in
                rows.into_iter().enumerate()
            {
                body.parameter_order.push(BoundaryParameterKind::Structural);
                body.structural_parameters
                    .push(StructuralParameterDeclaration {
                        place,
                        position: position as u32,
                        structural_type,
                        access,
                        multiplicity: if structural_type == boolean {
                            StructuralMultiplicity::Unrestricted
                        } else {
                            StructuralMultiplicity::Affine
                        },
                        qualifications: if promises_domain && structural_type == boolean {
                            vec![domain]
                        } else {
                            Vec::new()
                        },
                        projected_qualifications: if promises_domain && structural_type == record {
                            vec![StructuralPathQualification {
                                path: selected_path.clone(),
                                domain,
                            }]
                        } else {
                            Vec::new()
                        },
                    });
                body.structural_places.push(StructuralPlaceDeclaration {
                    id: place,
                    kind: StructuralPlaceKind::Parameter {
                        position: position as u32,
                    },
                });
            }
        };

    for writers in [
        Writers::PromisedReplacement,
        Writers::AdditionalUnknownReplacement,
        Writers::AdditionalDeeperUnknownReplacement,
    ] {
        let mut caller = unit_machine();
        caller.id = caller_id;
        // An owned Boolean parameter has a concrete caller storage home. The
        // captured reference selects that home rather than an incoming pointee.
        parameters(
            &mut caller,
            vec![
                (old, boolean, StructuralAccess::Owned, true),
                (replacement, boolean, StructuralAccess::Owned, true),
                (unknown, boolean, StructuralAccess::Owned, false),
            ],
        );
        for (ordinal, place, structural_type, projected_qualifications, kind) in [
            (
                1,
                pointer,
                reference,
                vec![StructuralPathQualification {
                    path: vec![StructuralPathSegment::Referent],
                    domain,
                }],
                OperationKind::EstablishReference {
                    source: argument(old, StructuralAccess::MutableBorrow, Vec::new()),
                    storage: None,
                },
            ),
            (
                2,
                captured,
                record,
                vec![StructuralPathQualification {
                    path: selected_path.clone(),
                    domain,
                }],
                OperationKind::EstablishRecord {
                    fields: vec![RecordFieldInitializer {
                        field,
                        value: RecordFieldValue::Structural(argument(
                            pointer,
                            StructuralAccess::Owned,
                            Vec::new(),
                        )),
                    }],
                },
            ),
        ] {
            let operation = OperationId::new(ordinal).unwrap();
            caller.structural_places.push(StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::OperationResult {
                    producer: operation,
                    structural_type,
                },
            });
            caller.blocks[0].operations.push(Operation::unsuspended(
                operation,
                terminal_psi::OperationResult::Structural(StructuralOperationResult {
                    place,
                    structural_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    qualifications: Vec::new(),
                    projected_qualifications,
                    qualification_establishments: Vec::new(),
                    claims: Vec::new(),
                }),
                kind,
            ));
        }
        caller.blocks[0].operations.push(Operation::unsuspended(
            OperationId::new(3).unwrap(),
            terminal_psi::OperationResult::Unit,
            call(
                writer_id,
                vec![
                    argument(captured, StructuralAccess::Owned, Vec::new()),
                    argument(replacement, StructuralAccess::Owned, Vec::new()),
                    argument(unknown, StructuralAccess::Owned, Vec::new()),
                ],
            ),
        ));

        let mut writer = unit_machine();
        writer.id = writer_id;
        writer.contract = terminal_psi::MachineContract::empty(ContractId::new(2).unwrap());
        parameters(
            &mut writer,
            vec![
                (formal_record, record, StructuralAccess::Owned, true),
                (formal_replacement, boolean, StructuralAccess::Owned, true),
                (formal_unknown, boolean, StructuralAccess::Owned, false),
            ],
        );
        let store = |destination, path, value| OperationKind::StoreStructuralField {
            disposition: StructuralStoreDisposition::ReplaceDiscardable,
            destination,
            path,
            field: None,
            value: argument(value, StructuralAccess::Owned, Vec::new()),
        };
        writer.blocks[0].operations.push(Operation::unsuspended(
            OperationId::new(1).unwrap(),
            terminal_psi::OperationResult::Unit,
            store(formal_record, selected_path.clone(), formal_replacement),
        ));
        match writers {
            Writers::PromisedReplacement => {}
            Writers::AdditionalUnknownReplacement => {
                writer.blocks[0].operations.push(Operation::unsuspended(
                    OperationId::new(2).unwrap(),
                    terminal_psi::OperationResult::Unit,
                    store(formal_record, selected_path.clone(), formal_unknown),
                ));
            }
            Writers::AdditionalDeeperUnknownReplacement => {
                writer.blocks[0].operations.push(Operation::unsuspended(
                    OperationId::new(2).unwrap(),
                    terminal_psi::OperationResult::Unit,
                    call(
                        deeper_id,
                        vec![
                            argument(
                                formal_record,
                                StructuralAccess::MutableBorrow,
                                selected_path.clone(),
                            ),
                            argument(formal_unknown, StructuralAccess::Owned, Vec::new()),
                        ],
                    ),
                ));
            }
        }
        let mut machines = vec![caller, writer];
        if writers == Writers::AdditionalDeeperUnknownReplacement {
            let mut deeper = unit_machine();
            deeper.id = deeper_id;
            deeper.contract = terminal_psi::MachineContract::empty(ContractId::new(3).unwrap());
            parameters(
                &mut deeper,
                vec![
                    (
                        deeper_destination,
                        boolean,
                        StructuralAccess::MutableBorrow,
                        true,
                    ),
                    (deeper_unknown, boolean, StructuralAccess::Owned, false),
                ],
            );
            deeper.blocks[0].operations.push(Operation::unsuspended(
                OperationId::new(1).unwrap(),
                terminal_psi::OperationResult::Unit,
                store(deeper_destination, Vec::new(), deeper_unknown),
            ));
            machines.push(deeper);
        }
        let types = [
            (
                boolean,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            ),
            (
                reference,
                StructuralTypeShape::Reference {
                    referent: boolean,
                    access: StructuralAccess::MutableBorrow,
                },
            ),
            (
                record,
                StructuralTypeShape::Record {
                    storage: StructuralRecordStorage::Packed,
                    fields: vec![StructuralFieldDeclaration {
                        id: field,
                        identity: field_name,
                        inline_carrier: None,
                        relevance: BindingRelevance::Relevant,
                        storage: None,
                        field_type: StructuralFieldType::Structural(reference),
                    }],
                },
            ),
        ]
        .into_iter()
        .map(|(id, shape)| StructuralTypeDeclaration {
            id,
            identity: TerminalIdentityIndex(id.get() as u32),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            copyable: id == boolean,
            descriptor_cleanup: id != boolean,
            scalar_restriction: None,
            shape,
        })
        .collect();
        let subject = PlaceId::new(99).unwrap();
        let module = TerminalModule {
            structural_types: types,
            structural_domains: vec![StructuralDomainDeclaration {
                id: domain,
                semantic_domain: DomainSemanticId::new(1).unwrap(),
                identity: TerminalIdentityIndex(10),
                carrier: boolean,
                required_memberships: Some(Vec::new()),
                complete_predicate_theory: true,
                predicate: Some(StructuralDomainPredicate {
                    subject,
                    proposition: Proposition::Equal(
                        ScalarTerm::boolean_field_path(subject, Vec::new()),
                        ScalarTerm::Boolean(true),
                    ),
                }),
                content_projection: None,
                byte_predicate: None,
                establishment_routes: Vec::new(),
            }],
            machines,
            ..TerminalModule::declarations()
        };
        let graph = terminal_borrowed_storage_graph_from(
            &module,
            module.machines.iter().map(|machine| machine.id),
        );
        assert_eq!(
            graph.len(),
            module.machines.len(),
            "{writers:?}: typed graph adapts"
        );
        let call_position = (module.machines[0].entry, 2);
        let mut analysis =
            DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
        let mut context = NormalPayloadContext::new(&module, &graph);
        let effects = context
            .call_write_effects(&mut analysis, caller_id, call_position)
            .expect("the real direct invocation has complete executable write effects");
        assert!(
            !effects.is_empty(),
            "{writers:?}: the store cannot disappear"
        );
        let first_position = (module.machines[1].entry, 0);
        let mut writer_history =
            PayloadMembershipHistory::for_module(&module, &graph, &module.machines[1]);
        let promised_rhs = writer_history
            .current_selected_payload_contract_in(
                &mut analysis,
                first_position,
                formal_replacement,
                &[],
            )
            .expect("the independent RHS has its own pre-store observed contract");
        assert!(
            promised_rhs.qualifications.contains(&domain),
            "{writers:?}: D is already promised by the RHS, not the destination"
        );
        let first_destination = DescriptorLocation {
            root: formal_record,
            path: descriptor_selected_path.clone(),
        };
        assert!(
            effects.iter().any(|effect| effect
                .performed
                .as_ref()
                .ok()
                .and_then(write_conservation::PerformedWrite::payload)
                .is_some_and(|performed| {
                    performed.machine == writer_id
                        && performed.position == first_position
                        && performed.source == DescriptorLocation::whole(formal_replacement)
                        && performed.destination == first_destination
                })),
            "{writers:?}: the promising writer retains its actual operation identity"
        );
        for effect in &effects {
            assert_eq!(
                analysis.exact_payload_target(caller_id, call_position, &effect.query),
                Some(DescriptorLocation::whole(old)),
                "{writers:?}: every writer selects the original Boolean home"
            );
        }
        if writers != Writers::PromisedReplacement {
            let (identity, position, source) = match writers {
                Writers::AdditionalUnknownReplacement => {
                    (writer_id, (module.machines[1].entry, 1), formal_unknown)
                }
                Writers::AdditionalDeeperUnknownReplacement => {
                    (deeper_id, (module.machines[2].entry, 0), deeper_unknown)
                }
                Writers::PromisedReplacement => unreachable!(),
            };
            assert!(
                effects.iter().any(|effect| effect
                    .performed
                    .as_ref()
                    .ok()
                    .and_then(write_conservation::PerformedWrite::payload)
                    .is_some_and(|performed| {
                        performed.machine == identity
                            && performed.position == position
                            && performed.source == DescriptorLocation::whole(source)
                    })),
                "{writers:?}: another writer of the same home cannot be deduplicated away"
            );
            let unknown_body = module
                .machines
                .iter()
                .find(|body| body.id == identity)
                .unwrap();
            let mut unknown_history =
                PayloadMembershipHistory::for_module(&module, &graph, unknown_body);
            match unknown_history.current_selected_payload_contract_in(
                &mut analysis,
                position,
                source,
                &[],
            ) {
                Ok(unknown_rhs) => assert!(
                    !unknown_rhs.qualifications.contains(&domain),
                    "{writers:?}: Observed closure must not independently prove Boolean == true"
                ),
                Err(reason) => assert!(
                    !reason.is_empty(),
                    "{writers:?}: unavailable pre-store history must retain its refusal reason"
                ),
            }
        }
        let original = PayloadMembershipContract {
            carrier: boolean,
            qualifications: vec![domain],
            projected_qualifications: Vec::new(),
            scalar_observations: vec![PayloadScalarObservation {
                path: Vec::new(),
                value: Some(ScalarTerm::Boolean(true)),
                entry: None,
            }],
            scalar_requirements: vec![Proposition::Equal(
                ScalarTerm::boolean_field_path(old, Vec::new()),
                ScalarTerm::Boolean(true),
            )],
        };
        let before = State {
            values: [(old, original)].into(),
            ..State::default()
        };
        // Supply the already-retired post-call cut. No performed-write proof is
        // constructed here: every proof above comes from the Terminal operation.
        let mut after = State {
            values: [(
                old,
                PayloadMembershipContract {
                    carrier: boolean,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    scalar_observations: Vec::new(),
                    scalar_requirements: Vec::new(),
                },
            )]
            .into(),
            unavailable_preservation: vec![old],
            ..State::default()
        };
        let conserved = context
            .conserved_call_payloads(&mut analysis, caller_id, call_position, &before)
            .expect("all writer identities and exact storage targets are available");
        let should_conserve = writers == Writers::PromisedReplacement;
        assert_eq!(conserved.len(), usize::from(should_conserve), "{writers:?}");
        for payload in conserved {
            payload.restore(&mut after);
        }
        let current = after.values.get(&old).unwrap();
        assert_eq!(
            current.qualifications,
            if should_conserve {
                vec![domain]
            } else {
                Vec::new()
            },
            "{writers:?}"
        );
        assert_eq!(
            after.unavailable_preservation.contains(&old),
            !should_conserve,
            "{writers:?}"
        );
        assert!(
            current.scalar_observations.is_empty(),
            "{writers:?}: old scalar contents stay retired"
        );
        assert!(
            current.scalar_requirements.is_empty(),
            "{writers:?}: old field equalities stay retired"
        );
        assert!(
            after.scalar_requirements.is_empty(),
            "{writers:?}: ambient equalities stay retired"
        );
        assert!(
            after.shared_requirements.is_empty(),
            "{writers:?}: shared equalities stay retired"
        );
    }
}
