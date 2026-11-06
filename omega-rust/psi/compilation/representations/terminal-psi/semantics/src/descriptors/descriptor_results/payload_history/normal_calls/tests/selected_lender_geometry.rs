//! A provider's write to one field must not silently erase another field's rule.
//! Ordinary records separate their fields; an overlaid access view needs an
//! independent geometry proof before the selected-lender check can assume that.
use super::*;
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ContractId, EdgeId, OperationId, StructuralFieldId, ValueId,
};
use terminal_psi::{
    BindingRelevance, BoundaryCompletion, BoundaryDeclarationRole, BoundaryMachineDeclaration,
    BoundaryMachineResult, BoundaryParameterKind, BoundaryRequirementKind,
    NominalDeclarationIdentity, StructuralDomainPredicate, StructuralDomainRequirement,
    StructuralFieldDeclaration, StructuralFieldType, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPathQualification, StructuralPlaceDeclaration,
    StructuralRecordStorage,
};

#[test]
fn selected_lender_cannot_assume_overlaid_sibling_fields_are_disjoint() {
    let owner = PlaceId::new(1).unwrap();
    let pointer = PlaceId::new(2).unwrap();
    let formal = PlaceId::new(3).unwrap();
    let boolean = StructuralTypeId::new(1).unwrap();
    let record = StructuralTypeId::new(2).unwrap();
    let reference = StructuralTypeId::new(3).unwrap();
    let first = StructuralFieldId::new(1).unwrap();
    let second = StructuralFieldId::new(2).unwrap();
    let first_name = TerminalIdentityIndex(11);
    let second_name = TerminalIdentityIndex(12);
    let domain = StructuralDomainId::new(1).unwrap();
    let boundary = BoundaryMachineId::new(1).unwrap();
    let provider_id = MachineId::new(2).unwrap();
    let rhs = ValueId::new(1).unwrap();
    let receiving_path = vec![
        StructuralPathSegment::Referent,
        StructuralPathSegment::Field(second_name),
    ];
    let writer_path = vec![
        StructuralPathSegment::Referent,
        StructuralPathSegment::Field(first_name),
    ];

    for overlaid in [false, true] {
        let storage = if overlaid {
            StructuralRecordStorage::AccessView(semantic_vocabulary::AuthoredRecordStorage {
                byte_size: 1,
                byte_alignment: 1,
            })
        } else {
            StructuralRecordStorage::Packed
        };
        let types = [
            (
                boolean,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            ),
            (
                record,
                StructuralTypeShape::Record {
                    storage,
                    fields: [(first, first_name), (second, second_name)]
                        .into_iter()
                        .map(|(id, identity)| StructuralFieldDeclaration {
                            id,
                            identity,
                            inline_carrier: Some(boolean),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                            storage: overlaid.then_some(
                                semantic_vocabulary::AuthoredFieldStorage::Scalar {
                                    byte_offset: 0,
                                    stored_scalar: Some(ScalarType::Boolean),
                                },
                            ),
                        })
                        .collect(),
                },
            ),
            (
                reference,
                StructuralTypeShape::Reference {
                    referent: record,
                    access: StructuralAccess::MutableBorrow,
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
            copyable: id != reference,
            descriptor_cleanup: id == reference,
            scalar_restriction: None,
            shape,
        })
        .collect();
        let subject = PlaceId::new(99).unwrap();
        let domains = vec![StructuralDomainDeclaration {
            id: domain,
            semantic_domain: DomainSemanticId::new(1).unwrap(),
            identity: TerminalIdentityIndex(20),
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

        let mut caller = unit_machine();
        caller.parameter_order = vec![BoundaryParameterKind::Structural];
        caller.structural_parameters = vec![StructuralParameterDeclaration {
            place: owner,
            position: 0,
            structural_type: record,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: vec![StructuralPathQualification {
                path: vec![StructuralPathSegment::Field(second_name)],
                domain,
            }],
        }];
        caller.structural_places = vec![
            StructuralPlaceDeclaration {
                id: owner,
                kind: StructuralPlaceKind::Parameter { position: 0 },
            },
            StructuralPlaceDeclaration {
                id: pointer,
                kind: StructuralPlaceKind::OperationResult {
                    producer: OperationId::new(1).unwrap(),
                    structural_type: reference,
                },
            },
        ];
        caller.blocks[0].operations = vec![
            Operation::unsuspended(
                OperationId::new(1).unwrap(),
                terminal_psi::OperationResult::Structural(StructuralOperationResult {
                    place: pointer,
                    structural_type: reference,
                    multiplicity: StructuralMultiplicity::Affine,
                    qualifications: Vec::new(),
                    projected_qualifications: vec![StructuralPathQualification {
                        path: receiving_path.clone(),
                        domain,
                    }],
                    qualification_establishments: Vec::new(),
                    claims: Vec::new(),
                }),
                OperationKind::EstablishReference {
                    source: StructuralArgument::whole(owner, StructuralAccess::MutableBorrow),
                    storage: None,
                },
            ),
            Operation::unsuspended(
                OperationId::new(2).unwrap(),
                terminal_psi::OperationResult::Unit,
                OperationKind::BoundaryCall {
                    boundary,
                    arguments: Vec::new(),
                    structural_arguments: vec![StructuralArgument::whole(
                        pointer,
                        StructuralAccess::MutableBorrow,
                    )],
                    completion_receipts: Vec::new(),
                    requirement_obligations: Vec::new(),
                },
            ),
            Operation::unsuspended(
                OperationId::new(3).unwrap(),
                terminal_psi::OperationResult::Unit,
                OperationKind::ReleaseReference {
                    source: pointer,
                    path: Vec::new(),
                },
            ),
        ];
        let declaration = BoundaryMachineDeclaration {
            id: boundary,
            declaration: None,
            numeric_intrinsic: None,
            role: BoundaryDeclarationRole::Callable,
            kind: BoundaryRequirementKind::Machine,
            completion: BoundaryCompletion::NormalReturn,
            unconditional_termination: false,
            identity: TerminalIdentityIndex(21),
            owner: NominalDeclarationIdentity::new(TerminalIdentityIndex(1)),
            attachment: None,
            parameter_order: vec![BoundaryParameterKind::Structural],
            scalar_parameters: Vec::new(),
            crash_routes: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: formal,
                position: 0,
                structural_type: reference,
                multiplicity: StructuralMultiplicity::Affine,
                access: StructuralAccess::MutableBorrow,
                qualifications: Vec::new(),
                projected_qualifications: vec![StructuralPathQualification {
                    path: receiving_path.clone(),
                    domain,
                }],
            }],
            result: BoundaryMachineResult::Unit,
            result_reference_regions: Vec::new(),
            requires: Vec::new(),
            normal_qualifications: vec![StructuralDomainRequirement {
                argument_index: 0,
                path: receiving_path.clone(),
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
        let mut provider = unit_machine();
        provider.id = provider_id;
        provider.entry = BlockId::new(2).unwrap();
        provider.blocks[0].id = provider.entry;
        provider.blocks[0].terminator = Terminator::ReturnUnit {
            edge: EdgeId::new(2).unwrap(),
            cleanup_actions: Vec::new(),
        };
        provider.contract = terminal_psi::MachineContract::empty(ContractId::new(2).unwrap());
        provider.parameter_order = declaration.parameter_order.clone();
        provider.structural_parameters = declaration.structural_parameters.clone();
        provider.structural_places = vec![StructuralPlaceDeclaration {
            id: formal,
            kind: StructuralPlaceKind::Parameter { position: 0 },
        }];
        provider.blocks[0].operations = vec![
            Operation::unsuspended(
                OperationId::new(4).unwrap(),
                terminal_psi::OperationResult::Scalar(terminal_psi::ValueDeclaration {
                    id: rhs,
                    scalar_type: ScalarType::Boolean,
                    qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
                }),
                OperationKind::BooleanConstant { value: false },
            ),
            Operation::unsuspended(
                OperationId::new(5).unwrap(),
                terminal_psi::OperationResult::Unit,
                OperationKind::WriteOnlyPrimitiveStore {
                    destination: formal,
                    path: writer_path.clone(),
                    value: rhs,
                    range_obligation: None,
                },
            ),
        ];
        let demand = terminal_psi::BoundaryLenderPreservationDemand {
            machine: caller.id,
            operation: OperationId::new(2).unwrap(),
            requirement: declaration.normal_qualifications[0].clone(),
        };
        let module = TerminalModule {
            structural_types: types,
            structural_domains: domains,
            machines: vec![caller, provider],
            boundary_machines: vec![declaration],
            boundary_lender_preservation: vec![demand],
            ..TerminalModule::declarations()
        };
        let caller = &module.machines[0];
        let position = (caller.entry, 1);
        let graph = terminal_borrowed_storage_graph_from(&module, [caller.id, provider_id]);
        let caller_graph = descriptor_machine(&graph, caller.id).expect("actual caller graph");
        let mut analysis =
            DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
        let mut history = PayloadMembershipHistory::for_module(&module, &graph, caller);
        let original = history
            .current_payload_contract_in(&mut analysis, position, owner)
            .expect("the real pre-call history retains the entry field duty");
        assert!(
            original
                .projected_qualifications
                .contains(&StructuralPathQualification {
                    path: vec![StructuralPathSegment::Field(second_name)],
                    domain,
                })
        );
        let before = State {
            values: [(owner, original)].into(),
            ..State::default()
        };
        let mut selected_subject = caller_graph.calls[&position].argument_contents[0]
            .as_ref()
            .expect("actual captured argument contents")
            .clone();
        let suffix = descriptor_contents_query(
            &module.structural_types,
            formal,
            reference,
            &receiving_path,
            None,
        )
        .expect("typed public Referent/field-B subject");
        selected_subject.steps.extend(suffix.steps);
        assert_eq!(
            analysis.exact_payload_target(caller.id, position, &selected_subject),
            Some(DescriptorLocation {
                root: owner,
                path: vec![DescriptorPathSegment::Field(second)],
            }),
            "the retained duty is the actual field-B subject",
        );
        let mut context = NormalPayloadContext::new(&module, &graph);
        assert_eq!(
            context
                .captured_provider_facts(&mut analysis, caller.id, position, &before)
                .expect("real prior membership and the requested Referent subject")
                .len(),
            1,
        );
        let writes = context
            .body_writes(&graph, &mut analysis, provider_id)
            .expect("the actual provider has one complete primitive writer");
        assert_eq!(writes.len(), 1);
        assert!(matches!(
            &writes[0].performed,
            Ok(super::super::write_conservation::PerformedWrite::Primitive(
                _
            )),
        ));
        let mut actual_write = caller_graph.calls[&position].argument_contents[writes[0].parameter]
            .as_ref()
            .expect("actual captured writer argument")
            .clone();
        actual_write.steps.extend_from_slice(&writes[0].steps);
        assert_eq!(
            analysis.unchanged_payload_targets(caller.id, position, &actual_write),
            Some(vec![DescriptorLocation {
                root: owner,
                path: vec![DescriptorPathSegment::Field(first)],
            }]),
            "the performed False store reaches actual field A",
        );
        assert!(
            context
                .require_provider_preservation(&mut analysis, caller.id, position, &before, None)
                .is_err(),
            "the public promise supplies no selected-body proof"
        );
        let preservation = context.require_provider_preservation(
            &mut analysis,
            caller.id,
            position,
            &before,
            Some(provider_id),
        );
        if overlaid {
            assert_eq!(
                preservation,
                Err("scalar preservation needs independently proved external record geometry"),
                "overlaid sibling storage needs independent geometry before preserving field B",
            );
        } else {
            preservation.expect("a Packed field-A write leaves the distinct field-B duty intact");
        }
    }
}
