use super::*;
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ContractId, EdgeId, OperationId, ScalarQualificationSetId,
    ScalarType, StructuralPlaceKind, ValueId,
};
use terminal_psi::{
    BoundaryCompletion, BoundaryDeclarationRole, BoundaryLifetimeDependency, BoundaryParameterKind,
    BoundaryReferenceOutput, BoundaryRequirementKind, NominalDeclarationIdentity, OperationResult,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralStoreDisposition, SuccessorEdge, TerminalIdentityIndex, ValueDeclaration,
};

fn parameter(place: u64, position: u32, carrier: u64) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: PlaceId::new(place).unwrap(),
        position,
        structural_type: StructuralTypeId::new(carrier).unwrap(),
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn module() -> TerminalModule {
    let boundary = BoundaryMachineId::new(1).unwrap();
    let caller_id = MachineId::new(1).unwrap();
    let implementation = MachineId::new(2).unwrap();
    let boolean = StructuralTypeId::new(1).unwrap();
    let reference = StructuralTypeId::new(2).unwrap();
    let choice = ValueId::new(1).unwrap();
    let mut body = super::super::tests::unit_machine();
    body.id = implementation;
    body.contract = terminal_psi::MachineContract::empty(ContractId::new(2).unwrap());
    body.parameters = vec![ValueDeclaration {
        id: choice,
        scalar_type: ScalarType::Boolean,
        qualifications: ScalarQualificationSetId::ZERO,
    }];
    body.structural_parameters = vec![parameter(1, 0, 1), parameter(2, 1, 1), parameter(3, 2, 2)];
    body.parameter_order = vec![
        BoundaryParameterKind::Structural,
        BoundaryParameterKind::Structural,
        BoundaryParameterKind::Structural,
        BoundaryParameterKind::Scalar,
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
    let edge = |ordinal, block| SuccessorEdge {
        edge: EdgeId::new(ordinal).unwrap(),
        target: BlockId::new(block).unwrap(),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        cleanup_actions: Vec::new(),
    };
    body.blocks[0].terminator = Terminator::Conditional {
        condition: choice,
        when_true: edge(1, 2),
        when_false: edge(2, 3),
    };
    for (ordinal, block, source) in [(1, 2, 1), (3, 3, 2)] {
        let pointer = PlaceId::new(10 + ordinal).unwrap();
        let producer = OperationId::new(ordinal).unwrap();
        body.structural_places.push(StructuralPlaceDeclaration {
            id: pointer,
            kind: StructuralPlaceKind::OperationResult {
                producer,
                structural_type: reference,
            },
        });
        let mut branch = terminal_psi::Block::plain(
            BlockId::new(block).unwrap(),
            Vec::new(),
            Terminator::ReturnUnit {
                edge: EdgeId::new(block + 1).unwrap(),
                cleanup_actions: Vec::new(),
            },
        );
        branch.operations.push(Operation::unsuspended(
            producer,
            OperationResult::Structural(StructuralOperationResult {
                place: pointer,
                structural_type: reference,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            }),
            OperationKind::EstablishReference {
                source: StructuralArgument::whole(
                    PlaceId::new(source).unwrap(),
                    StructuralAccess::MutableBorrow,
                ),
                storage: None,
            },
        ));
        branch.operations.push(Operation::unsuspended(
            OperationId::new(ordinal + 1).unwrap(),
            OperationResult::Unit,
            OperationKind::StoreStructuralField {
                disposition: StructuralStoreDisposition::ReplaceDiscardable,
                destination: PlaceId::new(3).unwrap(),
                path: Vec::new(),
                field: None,
                value: StructuralArgument::whole(pointer, StructuralAccess::Owned),
            },
        ));
        body.blocks.push(branch);
    }
    let mut caller = super::super::tests::unit_machine();
    caller.parameters = body.parameters.clone();
    caller.structural_parameters = body.structural_parameters.clone();
    caller.parameter_order = body.parameter_order.clone();
    caller.structural_places = caller
        .structural_parameters
        .iter()
        .map(|parameter| StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
            },
        })
        .collect();
    caller.blocks[0].operations.push(Operation::unsuspended(
        OperationId::new(1).unwrap(),
        OperationResult::Unit,
        OperationKind::BoundaryCall {
            boundary,
            arguments: vec![choice],
            structural_arguments: caller
                .structural_parameters
                .iter()
                .map(|parameter| StructuralArgument::whole(parameter.place, parameter.access))
                .collect(),
            completion_receipts: Vec::new(),
            requirement_obligations: Vec::new(),
        },
    ));
    let declaration = BoundaryMachineDeclaration {
        id: boundary,
        declaration: None,
        role: BoundaryDeclarationRole::Callable,
        kind: BoundaryRequirementKind::Machine,
        completion: BoundaryCompletion::NormalReturn,
        numeric_intrinsic: None,
        unconditional_termination: false,
        identity: TerminalIdentityIndex(10),
        owner: NominalDeclarationIdentity::new(TerminalIdentityIndex(11)),
        attachment: None,
        scalar_parameters: vec![ScalarType::Boolean],
        parameter_order: body.parameter_order.clone(),
        crash_routes: Vec::new(),
        structural_parameters: body.structural_parameters.clone(),
        result: BoundaryMachineResult::Unit,
        requires: Vec::new(),
        normal_qualifications: Vec::new(),
        scalar_domain_requirements: Vec::new(),
        scalar_requires: Vec::new(),
        scalar_ensures: Vec::new(),
        result_reference_regions: Vec::new(),
        reference_outputs: vec![BoundaryReferenceOutput {
            destination: BoundaryReferenceDestination::Parameter(2),
            path: Vec::new(),
            access: StructuralAccess::MutableBorrow,
            dependencies: vec![BoundaryLifetimeDependency::Stored {
                argument_index: 2,
                path: Vec::new(),
                access: StructuralAccess::MutableBorrow,
            }],
        }],
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        fixed_service_reach: Vec::new(),
        owning_service: None,
        published_service_ceiling: Vec::new(),
    };
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
    ]
    .into_iter()
    .map(|(id, shape)| StructuralTypeDeclaration {
        id,
        identity: TerminalIdentityIndex(id.get() as u32),
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        copyable: id == boolean,
        descriptor_cleanup: false,
        scalar_restriction: None,
        shape,
    })
    .collect();
    TerminalModule {
        machines: vec![caller, body],
        structural_types: types,
        boundary_machines: vec![declaration],
        boundary_reference_forwarding: vec![BoundaryReferenceForwardingDemand {
            machine: caller_id,
            operation: OperationId::new(1).unwrap(),
            output: 0,
            sources: vec![
                BoundaryReferenceSource::Reference { argument_index: 0 },
                BoundaryReferenceSource::Reference { argument_index: 1 },
            ],
        }],
        ..TerminalModule::declarations()
    }
}

#[test]
fn forwarding_requires_every_normal_exit_source_and_an_actual_body() {
    let original = module();
    let boundary = original.boundary_machines[0].id;
    let body = original.machines[1].id;
    assert_eq!(require(&original, boundary, Some(body)), Ok(()));
    // Forwarding-only requests must reach the same public selection gate.
    assert_eq!(
        super::super::require_boundary_lender_preservation(&original, boundary, Some(body)),
        Ok(())
    );
    let mut omitted = original.clone();
    omitted.boundary_reference_forwarding[0].sources.pop();
    assert_eq!(
        require(&omitted, boundary, Some(body)),
        Err("a normal output reference is not one of the captured sources")
    );
    assert_eq!(
        require(&original, boundary, None),
        Err("reference forwarding requires an independently checked execution body")
    );
}

#[test]
fn forwarding_checks_untouched_output_slots_in_addition_to_written_effects() {
    let mut module = module();
    let boundary = module.boundary_machines[0].id;
    let body = module.machines[1].id;
    module.machines[1].blocks[2].operations.pop();
    assert_eq!(
        require(&module, boundary, Some(body)),
        Err("a normal output reference is not one of the captured sources")
    );
    module.boundary_reference_forwarding[0]
        .sources
        .push(BoundaryReferenceSource::Stored {
            argument_index: 2,
            path: Vec::new(),
        });
    assert_eq!(require(&module, boundary, Some(body)), Ok(()));
}

fn add_source_duty(module: &mut TerminalModule) -> semantic_vocabulary::StructuralDomainId {
    let domain = semantic_vocabulary::StructuralDomainId::new(1).unwrap();
    module
        .structural_domains
        .push(terminal_psi::StructuralDomainDeclaration {
            id: domain,
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(1).unwrap(),
            identity: TerminalIdentityIndex(20),
            carrier: StructuralTypeId::new(1).unwrap(),
            content_projection: None,
            byte_predicate: None,
            required_memberships: Some(Vec::new()),
            complete_predicate_theory: true,
            predicate: None,
            establishment_routes: Vec::new(),
        });
    domain
}

fn qualify_forwarded_pointer(
    module: &mut TerminalModule,
    block: usize,
    domain: semantic_vocabulary::StructuralDomainId,
) {
    let OperationResult::Structural(result) =
        &mut module.machines[1].blocks[block].operations[0].result
    else {
        panic!("the fixture must form its forwarded pointer first");
    };
    result
        .projected_qualifications
        .push(terminal_psi::StructuralPathQualification {
            path: vec![StructuralPathSegment::Referent],
            domain,
        });
}

#[test]
fn forwarding_preserves_the_complete_captured_duties_on_every_exit() {
    let mut module = module();
    let domain = add_source_duty(&mut module);
    for machine in &mut module.machines {
        for parameter in &mut machine.structural_parameters[..2] {
            parameter.qualifications.push(domain);
        }
    }
    for parameter in &mut module.boundary_machines[0].structural_parameters[..2] {
        parameter.qualifications.push(domain);
    }
    qualify_forwarded_pointer(&mut module, 1, domain);
    qualify_forwarded_pointer(&mut module, 2, domain);
    assert_eq!(
        require(
            &module,
            module.boundary_machines[0].id,
            Some(module.machines[1].id)
        ),
        Ok(())
    );
}

#[test]
fn forwarding_rejects_an_added_duty_even_when_the_pointer_origin_is_unchanged() {
    let mut module = module();
    let domain = add_source_duty(&mut module);
    // Only the second exit adds a promise. Its pointer still comes from the
    // captured second input, so origin checking alone would accept it.
    qualify_forwarded_pointer(&mut module, 2, domain);
    assert_eq!(
        require(
            &module,
            module.boundary_machines[0].id,
            Some(module.machines[1].id)
        ),
        Err("a normal output reference changes its captured source duties")
    );
}

#[test]
fn forwarding_cannot_supply_hidden_duties_from_private_entry_assumptions() {
    let mut module = module();
    let domain = add_source_duty(&mut module);
    for parameter in &mut module.machines[1].structural_parameters[..2] {
        parameter.qualifications.push(domain);
    }
    qualify_forwarded_pointer(&mut module, 1, domain);
    qualify_forwarded_pointer(&mut module, 2, domain);
    assert_eq!(
        require(
            &module,
            module.boundary_machines[0].id,
            Some(module.machines[1].id)
        ),
        Err("reference forwarding source assumes duties absent from its public input")
    );
}

#[test]
fn forwarding_preserves_public_duties_through_weaker_private_stored_transport() {
    let mut module = module();
    let domain = add_source_duty(&mut module);
    let row = terminal_psi::StructuralPathQualification {
        path: vec![StructuralPathSegment::Referent],
        domain,
    };
    module.boundary_machines[0].structural_parameters[2]
        .projected_qualifications
        .push(row.clone());
    module.machines[0].structural_parameters[2]
        .projected_qualifications
        .push(row);
    module.boundary_reference_forwarding[0].sources = vec![BoundaryReferenceSource::Stored {
        argument_index: 2,
        path: Vec::new(),
    }];
    for branch in &mut module.machines[1].blocks[1..] {
        branch.operations.clear();
    }
    module.machines[1]
        .structural_places
        .retain(|place| matches!(place.kind, StructuralPlaceKind::Parameter { .. }));
    assert_eq!(
        require(
            &module,
            module.boundary_machines[0].id,
            Some(module.machines[1].id)
        ),
        Ok(())
    );
}

#[test]
fn forwarding_rejects_mutable_writes_that_break_public_duties_absent_from_private_entry() {
    let mut module = module();
    let domain = add_source_duty(&mut module);
    let subject = PlaceId::new(99).unwrap();
    module.structural_domains[0].predicate = Some(terminal_psi::StructuralDomainPredicate {
        subject,
        proposition: semantic_vocabulary::Proposition::Equal(
            semantic_vocabulary::ScalarTerm::boolean_field_path(subject, Vec::new()),
            semantic_vocabulary::ScalarTerm::Boolean(true),
        ),
    });
    let row = terminal_psi::StructuralPathQualification {
        path: vec![StructuralPathSegment::Referent],
        domain,
    };
    module.boundary_machines[0].structural_parameters[2]
        .projected_qualifications
        .push(row.clone());
    module.machines[0].structural_parameters[2]
        .projected_qualifications
        .push(row);
    module.boundary_reference_forwarding[0].sources = vec![BoundaryReferenceSource::Stored {
        argument_index: 2,
        path: Vec::new(),
    }];
    for branch in &mut module.machines[1].blocks[1..] {
        branch.operations.clear();
    }
    module.machines[1]
        .structural_places
        .retain(|place| matches!(place.kind, StructuralPlaceKind::Parameter { .. }));
    assert_eq!(
        require(
            &module,
            module.boundary_machines[0].id,
            Some(module.machines[1].id)
        ),
        Ok(())
    );
    // The pointer stays untouched, but its boolean no longer satisfies the
    // caller's promise. Bare private metadata must not hide that failed duty.
    let body = &mut module.machines[1];
    body.blocks[0].operations = vec![
        Operation::unsuspended(
            OperationId::new(90).unwrap(),
            OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(2).unwrap(),
                scalar_type: ScalarType::Boolean,
                qualifications: ScalarQualificationSetId::ZERO,
            }),
            OperationKind::BooleanConstant { value: false },
        ),
        Operation::unsuspended(
            OperationId::new(91).unwrap(),
            OperationResult::Unit,
            OperationKind::WriteOnlyPrimitiveStore {
                destination: PlaceId::new(3).unwrap(),
                path: vec![StructuralPathSegment::Referent],
                value: ValueId::new(2).unwrap(),
                range_obligation: None,
            },
        ),
    ];
    assert!(
        require(
            &module,
            module.boundary_machines[0].id,
            Some(module.machines[1].id)
        )
        .is_err()
    );
}

#[test]
fn forwarding_proves_private_entry_conditions_from_public_requirements_only() {
    let mut module = module();
    let condition = semantic_vocabulary::Proposition::Equal(
        semantic_vocabulary::ScalarTerm::value(ValueId::new(1).unwrap(), ScalarType::Boolean),
        semantic_vocabulary::ScalarTerm::Boolean(true),
    );
    module.machines[1].contract.requires.push(condition.clone());
    let boundary = module.boundary_machines[0].id;
    let body = module.machines[1].id;
    assert_eq!(
        require(&module, boundary, Some(body)),
        Err("reference forwarding body assumes a condition not proved by its public input")
    );
    module.boundary_machines[0].scalar_requires.push(condition);
    assert_eq!(require(&module, boundary, Some(body)), Ok(()));
}

#[test]
fn forwarding_can_use_the_actual_predicate_of_a_public_entry_membership() {
    let mut module = module();
    let domain = add_source_duty(&mut module);
    let subject = PlaceId::new(99).unwrap();
    let proposition = |root| {
        semantic_vocabulary::Proposition::Equal(
            semantic_vocabulary::ScalarTerm::boolean_field_path(root, Vec::new()),
            semantic_vocabulary::ScalarTerm::Boolean(true),
        )
    };
    module.structural_domains[0].predicate = Some(terminal_psi::StructuralDomainPredicate {
        subject,
        proposition: proposition(subject),
    });
    for machine in &mut module.machines {
        for parameter in &mut machine.structural_parameters[..2] {
            parameter.qualifications.push(domain);
        }
    }
    for parameter in &mut module.boundary_machines[0].structural_parameters[..2] {
        parameter.qualifications.push(domain);
    }
    qualify_forwarded_pointer(&mut module, 1, domain);
    qualify_forwarded_pointer(&mut module, 2, domain);
    module.machines[1]
        .contract
        .requires
        .push(proposition(PlaceId::new(1).unwrap()));
    let boundary = module.boundary_machines[0].id;
    let body = module.machines[1].id;
    assert_eq!(require(&module, boundary, Some(body)), Ok(()));
    module.structural_domains[0].predicate = None;
    assert_eq!(
        require(&module, boundary, Some(body)),
        Err("reference forwarding body assumes a condition not proved by its public input")
    );
}

#[test]
fn forwarding_needs_public_scalar_membership_before_using_its_predicate() {
    let mut module = module();
    let domain = semantic_vocabulary::ScalarDomainId::new(1).unwrap();
    let set = ScalarQualificationSetId::new(1);
    let subject = ValueId::new(1).unwrap();
    let condition = semantic_vocabulary::Proposition::Equal(
        semantic_vocabulary::ScalarTerm::value(subject, ScalarType::Boolean),
        semantic_vocabulary::ScalarTerm::Boolean(true),
    );
    module
        .scalar_qualifications
        .domains
        .push(terminal_psi::ScalarDomainDeclaration {
            id: domain,
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(1).unwrap(),
            identity: TerminalIdentityIndex(20),
            carrier: ScalarType::Boolean,
            required_memberships: Some(Vec::new()),
            predicate: Some(terminal_psi::ScalarDomainPredicate {
                subject,
                proposition: condition.clone(),
            }),
            establishment_routes: Vec::new(),
        });
    module
        .scalar_qualifications
        .sets
        .push(terminal_psi::ScalarQualificationSet {
            id: set,
            domains: vec![domain],
        });
    module.machines[1].parameters[0].qualifications = set;
    module.machines[1].contract.requires.push(condition);
    let boundary = module.boundary_machines[0].id;
    let body = module.machines[1].id;
    assert_eq!(
        require(&module, boundary, Some(body)),
        Err("reference forwarding body assumes scalar membership absent from its public input")
    );
    module.boundary_machines[0].scalar_domain_requirements.push(
        terminal_psi::ScalarDomainRequirement {
            argument_index: 0,
            domain,
        },
    );
    assert_eq!(require(&module, boundary, Some(body)), Ok(()));
    module.scalar_qualifications.domains[0].predicate = None;
    assert_eq!(
        require(&module, boundary, Some(body)),
        Err("reference forwarding body assumes a condition not proved by its public input")
    );
}

#[test]
fn forwarding_cannot_bind_an_extra_private_erased_formal() {
    let mut module = module();
    module.machines[1]
        .contract
        .erased_scalar_formals
        .push(ValueDeclaration {
            id: ValueId::new(9).unwrap(),
            scalar_type: ScalarType::Boolean,
            qualifications: ScalarQualificationSetId::ZERO,
        });
    assert_eq!(
        require(
            &module,
            module.boundary_machines[0].id,
            Some(module.machines[1].id)
        ),
        Err("reference forwarding body has erased formals absent from its public input")
    );
}

#[test]
fn forwarding_accepts_duties_in_explicit_public_input_requirements() {
    let mut module = module();
    let domain = add_source_duty(&mut module);
    for machine in &mut module.machines {
        for parameter in &mut machine.structural_parameters[..2] {
            parameter.qualifications.push(domain);
        }
    }
    module.boundary_machines[0].requires = (0..2)
        .map(|argument_index| terminal_psi::StructuralDomainRequirement {
            argument_index,
            path: Vec::new(),
            domain,
        })
        .collect();
    qualify_forwarded_pointer(&mut module, 1, domain);
    qualify_forwarded_pointer(&mut module, 2, domain);
    assert_eq!(
        require(
            &module,
            module.boundary_machines[0].id,
            Some(module.machines[1].id)
        ),
        Ok(())
    );
}

#[test]
fn forwarding_rejects_losing_a_captured_duty_on_one_exit() {
    let mut module = module();
    let domain = add_source_duty(&mut module);
    for machine in &mut module.machines {
        for parameter in &mut machine.structural_parameters[..2] {
            parameter.qualifications.push(domain);
        }
    }
    for parameter in &mut module.boundary_machines[0].structural_parameters[..2] {
        parameter.qualifications.push(domain);
    }
    qualify_forwarded_pointer(&mut module, 1, domain);
    assert_eq!(
        require(
            &module,
            module.boundary_machines[0].id,
            Some(module.machines[1].id)
        ),
        Err("a normal output reference changes its captured source duties")
    );
}

#[test]
fn forwarding_keeps_the_entry_duties_of_an_untouched_stored_source() {
    let mut module = module();
    let domain = add_source_duty(&mut module);
    for machine in &mut module.machines {
        machine.structural_parameters[0].qualifications.push(domain);
        machine.structural_parameters[2]
            .projected_qualifications
            .push(terminal_psi::StructuralPathQualification {
                path: vec![StructuralPathSegment::Referent],
                domain,
            });
    }
    module.boundary_machines[0].structural_parameters =
        module.machines[1].structural_parameters.clone();
    qualify_forwarded_pointer(&mut module, 1, domain);
    module.machines[1].blocks[2].operations.pop();
    module.boundary_reference_forwarding[0]
        .sources
        .push(BoundaryReferenceSource::Stored {
            argument_index: 2,
            path: Vec::new(),
        });
    assert_eq!(
        require(
            &module,
            module.boundary_machines[0].id,
            Some(module.machines[1].id)
        ),
        Ok(())
    );
}

#[test]
fn forwarding_does_not_hide_an_unknown_roster_behind_a_known_pointer() {
    let mut module = module();
    let domain = add_source_duty(&mut module);
    for machine in &mut module.machines {
        machine.structural_parameters[1].qualifications.push(domain);
    }
    module.boundary_machines[0].structural_parameters[1]
        .qualifications
        .push(domain);
    qualify_forwarded_pointer(&mut module, 2, domain);
    let body = &mut module.machines[1];
    for (index, edge) in [(1, 5), (2, 6)] {
        body.blocks[index].terminator = Terminator::Jump {
            edge: EdgeId::new(edge).unwrap(),
            target: BlockId::new(4).unwrap(),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            cleanup_actions: Vec::new(),
        };
    }
    body.blocks.push(terminal_psi::Block::plain(
        BlockId::new(4).unwrap(),
        Vec::new(),
        Terminator::ReturnUnit {
            edge: EdgeId::new(7).unwrap(),
            cleanup_actions: Vec::new(),
        },
    ));
    let graph = super::super::super::super::terminal_graph_with_forwarding(
        &module,
        DescriptorSlotKind::BorrowedStorage,
        &[(module.boundary_machines[0].id, module.machines[1].id)],
        &[],
    )
    .unwrap();
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    let origins = analysis
        .symbolic_source_origins(
            module.machines[1].id,
            (BlockId::new(4).unwrap(), 0),
            &DescriptorLocation::whole(PlaceId::new(3).unwrap()),
        )
        .unwrap();
    assert_eq!(
        origins.into_iter().collect::<BTreeSet<_>>(),
        [1, 2]
            .map(
                |source| DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument {
                    source: PlaceId::new(source).unwrap(),
                    projection: Projection::new(&[]),
                })
            )
            .into_iter()
            .collect()
    );
    let mut history = PayloadMembershipHistory::for_preservation(
        &module.structural_types,
        &module.structural_domains,
        &graph,
        &module.machines[1],
    );
    assert!(
        history
            .preserved_normal_exit_rosters_in(&mut analysis, &[PlaceId::new(3).unwrap()], None,)
            .is_err()
    );
    assert!(
        require(
            &module,
            module.boundary_machines[0].id,
            Some(module.machines[1].id)
        )
        .is_err()
    );
}

#[test]
fn forwarding_cannot_use_an_opaque_helpers_claim_as_its_own_proof() {
    let mut module = module();
    let boundary = module.boundary_machines[0].id;
    let body = module.machines[1].id;
    let nested = BoundaryMachineId::new(2).unwrap();
    let mut declaration = module.boundary_machines[0].clone();
    declaration.id = nested;
    declaration.identity = TerminalIdentityIndex(12);
    module.boundary_machines.push(declaration);
    let call = OperationId::new(90).unwrap();
    let actuals = module.machines[1]
        .structural_parameters
        .iter()
        .map(|parameter| StructuralArgument::whole(parameter.place, parameter.access))
        .collect();
    module.machines[1].blocks[2].operations.truncate(1);
    module.machines[1].blocks[2]
        .operations
        .push(Operation::unsuspended(
            call,
            OperationResult::Unit,
            OperationKind::BoundaryCall {
                boundary: nested,
                arguments: vec![ValueId::new(1).unwrap()],
                structural_arguments: actuals,
                completion_receipts: Vec::new(),
                requirement_obligations: Vec::new(),
            },
        ));
    let mut claim = module.boundary_reference_forwarding[0].clone();
    claim.machine = body;
    claim.operation = call;
    module.boundary_reference_forwarding.push(claim);
    assert_eq!(
        require(&module, boundary, Some(body)),
        Err("reference forwarding needs an unchanged input value, not fresh or projected backing")
    );
}

#[test]
fn forwarding_replay_distinguishes_outer_pointer_home_from_stored_pointer_value() {
    let mut module = module();
    let body_id = module.machines[1].id;
    let pointer_slot = PlaceId::new(3).unwrap();
    let outer = PlaceId::new(99).unwrap();
    let producer = OperationId::new(99).unwrap();
    let nested = StructuralTypeId::new(3).unwrap();
    let mut declaration = module.structural_types[1].clone();
    declaration.id = nested;
    declaration.identity = TerminalIdentityIndex(3);
    declaration.shape = StructuralTypeShape::Reference {
        referent: StructuralTypeId::new(2).unwrap(),
        access: StructuralAccess::MutableBorrow,
    };
    module.structural_types.push(declaration);
    let body = &mut module.machines[1];
    body.structural_places.push(StructuralPlaceDeclaration {
        id: outer,
        kind: StructuralPlaceKind::OperationResult {
            producer,
            structural_type: nested,
        },
    });
    body.blocks[0].operations.push(Operation::unsuspended(
        producer,
        OperationResult::Structural(StructuralOperationResult {
            place: outer,
            structural_type: nested,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        }),
        OperationKind::EstablishReference {
            source: StructuralArgument::whole(pointer_slot, StructuralAccess::MutableBorrow),
            storage: None,
        },
    ));
    let after_formation = (body.entry, 1);
    let graph = super::super::super::super::terminal_graph_with_forwarding(
        &module,
        DescriptorSlotKind::BorrowedStorage,
        &[],
        &[],
    )
    .unwrap();
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    let stored = analysis
        .symbolic_entry_source_origins(body_id, &DescriptorLocation::whole(pointer_slot))
        .unwrap();
    let borrowed = analysis
        .symbolic_source_origins(body_id, after_formation, &DescriptorLocation::whole(outer))
        .unwrap();
    assert_eq!(
        stored,
        vec![DescriptorSourceOrigin::IncomingSlot(
            DescriptorLocation::whole(pointer_slot)
        )]
    );
    assert_eq!(
        borrowed,
        vec![DescriptorSourceOrigin::Backing(
            DescriptorCallOrigin::Argument {
                source: pointer_slot,
                projection: Projection::new(&[]),
            }
        )]
    );
    assert_ne!(
        unchanged_origin(stored[0].clone()).unwrap(),
        unchanged_origin(borrowed[0].clone()).unwrap()
    );
}

#[test]
fn provider_forwarding_captures_outer_pointer_and_invocation_contents_separately() {
    let mut module = module();
    let boundary = module.boundary_machines[0].id;
    let pointer = StructuralTypeId::new(2).unwrap();
    let outer_pointer = StructuralTypeId::new(3).unwrap();
    let captured_outer = StructuralTypeId::new(4).unwrap();
    let mut ty = module.structural_types[1].clone();
    ty.id = outer_pointer;
    ty.identity = TerminalIdentityIndex(3);
    ty.shape = StructuralTypeShape::Reference {
        referent: pointer,
        access: StructuralAccess::SharedBorrow,
    };
    let mut capture_type = ty.clone();
    capture_type.id = captured_outer;
    capture_type.identity = TerminalIdentityIndex(4);
    capture_type.shape = StructuralTypeShape::Reference {
        referent: pointer,
        access: StructuralAccess::MutableBorrow,
    };
    module.structural_types.extend([ty, capture_type]);
    let place = |id| PlaceId::new(id).unwrap();
    let mut caller = super::super::tests::unit_machine();
    caller.structural_parameters = vec![
        parameter(1, 0, 1),
        parameter(2, 1, 1),
        parameter(3, 2, 2),
        parameter(4, 3, 3),
        parameter(5, 4, 2),
    ];
    caller.parameter_order = vec![BoundaryParameterKind::Structural; 5];
    caller.structural_places = caller
        .structural_parameters
        .iter()
        .map(|parameter| StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
            },
        })
        .collect();
    let reference = |operation, source, destination, carrier| {
        Operation::unsuspended(
            OperationId::new(operation).unwrap(),
            OperationResult::Structural(StructuralOperationResult {
                place: place(destination),
                structural_type: carrier,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            }),
            OperationKind::EstablishReference {
                source: StructuralArgument::whole(place(source), StructuralAccess::MutableBorrow),
                storage: None,
            },
        )
    };
    for (producer, destination, carrier) in
        [(1, 10, pointer), (3, 20, captured_outer), (4, 11, pointer)]
    {
        caller.structural_places.push(StructuralPlaceDeclaration {
            id: place(destination),
            kind: StructuralPlaceKind::OperationResult {
                producer: OperationId::new(producer).unwrap(),
                structural_type: carrier,
            },
        });
    }
    let store = |operation, source| {
        Operation::unsuspended(
            OperationId::new(operation).unwrap(),
            OperationResult::Unit,
            OperationKind::StoreStructuralField {
                disposition: StructuralStoreDisposition::ReplaceDiscardable,
                destination: place(3),
                path: Vec::new(),
                field: None,
                value: StructuralArgument::whole(place(source), StructuralAccess::Owned),
            },
        )
    };
    let call = OperationId::new(6).unwrap();
    caller.blocks[0].operations = vec![
        reference(1, 1, 10, pointer),
        store(2, 10),
        // Capture the outer pointer while slot3 contains the first pointer.
        reference(3, 3, 20, captured_outer),
        reference(4, 2, 11, pointer),
        store(5, 11),
        Operation::unsuspended(
            call,
            OperationResult::Unit,
            OperationKind::BoundaryCall {
                boundary,
                arguments: Vec::new(),
                structural_arguments: vec![
                    StructuralArgument {
                        place: place(20),
                        path: vec![StructuralPathSegment::Referent],
                        access: StructuralAccess::MutableBorrow,
                    },
                    StructuralArgument::whole(place(4), StructuralAccess::MutableBorrow),
                    StructuralArgument::whole(place(5), StructuralAccess::MutableBorrow),
                ],
                completion_receipts: Vec::new(),
                requirement_obligations: Vec::new(),
            },
        ),
        // A later store must not replace the call's captured Stored[] value.
        store(7, 10),
    ];
    let declaration = &mut module.boundary_machines[0];
    declaration.scalar_parameters.clear();
    declaration.parameter_order = vec![BoundaryParameterKind::Structural; 3];
    declaration.structural_parameters =
        vec![parameter(3, 0, 2), parameter(4, 1, 3), parameter(5, 2, 2)];
    declaration.reference_outputs = [(0, pointer), (1, outer_pointer), (2, pointer)]
        .into_iter()
        .map(|(argument, _)| BoundaryReferenceOutput {
            destination: BoundaryReferenceDestination::Parameter(argument),
            path: Vec::new(),
            // A shared output observes slot3 without exporting another
            // mutable write to the slot already written through formal0.
            access: if argument == 1 {
                StructuralAccess::SharedBorrow
            } else {
                StructuralAccess::MutableBorrow
            },
            dependencies: if argument == 1 {
                vec![BoundaryLifetimeDependency::Referent {
                    argument_index: 0,
                    access: StructuralAccess::SharedBorrow,
                }]
            } else {
                vec![BoundaryLifetimeDependency::Stored {
                    argument_index: 0,
                    path: Vec::new(),
                    access: StructuralAccess::MutableBorrow,
                }]
            },
        })
        .collect();
    let caller_id = caller.id;
    let block = caller.entry;
    module.machines = vec![caller];
    module.boundary_reference_forwarding = vec![
        BoundaryReferenceForwardingDemand {
            machine: caller_id,
            operation: call,
            output: 1,
            sources: vec![BoundaryReferenceSource::Reference { argument_index: 0 }],
        },
        BoundaryReferenceForwardingDemand {
            machine: caller_id,
            operation: call,
            output: 2,
            sources: vec![BoundaryReferenceSource::Stored {
                argument_index: 0,
                path: Vec::new(),
            }],
        },
    ];
    let graph = super::super::super::super::terminal_graph_with_forwarding(
        &module,
        DescriptorSlotKind::BorrowedStorage,
        &[],
        &module.boundary_reference_forwarding,
    )
    .unwrap();
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    let referent = |root| DescriptorContentsQuery {
        root: place(root),
        steps: vec![DescriptorContentsStep::Address(
            DescriptorPathSegment::Referent,
        )],
    };
    for cut in [(block, 6), (block, 7)] {
        assert_eq!(
            analysis.unchanged_payload_targets(caller_id, cut, &referent(4)),
            Some(vec![DescriptorLocation::whole(place(3))])
        );
        assert_eq!(
            analysis.unchanged_payload_targets(caller_id, cut, &referent(5)),
            Some(vec![DescriptorLocation::whole(place(2))])
        );
    }
    assert_eq!(
        analysis.unchanged_payload_targets(caller_id, (block, 7), &referent(3)),
        Some(vec![DescriptorLocation::whole(place(1))])
    );
    drop(analysis);

    // Export the provider through one ordinary helper summary. Its address
    // choice must not turn into a reload of that formal's stored pointer.
    let mut outer = super::super::tests::unit_machine();
    outer.id = MachineId::new(3).unwrap();
    outer.contract = terminal_psi::MachineContract::empty(ContractId::new(3).unwrap());
    outer.structural_parameters = module.machines[0].structural_parameters.clone();
    for parameter in &mut outer.structural_parameters {
        parameter.place = place(parameter.place.get() + 10);
    }
    outer.parameter_order = vec![BoundaryParameterKind::Structural; 5];
    outer.structural_places = outer
        .structural_parameters
        .iter()
        .map(|parameter| StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
            },
        })
        .collect();
    outer.blocks[0].operations.push(Operation::unsuspended(
        OperationId::new(100).unwrap(),
        OperationResult::Unit,
        OperationKind::CallUnit {
            callee: caller_id,
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: outer
                .structural_parameters
                .iter()
                .map(|parameter| StructuralArgument::whole(parameter.place, parameter.access))
                .collect(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    ));
    let outer_id = outer.id;
    let after_call = (outer.entry, 1);
    module.machines.push(outer);
    let graph = super::super::super::super::terminal_graph_with_forwarding(
        &module,
        DescriptorSlotKind::BorrowedStorage,
        &[],
        &module.boundary_reference_forwarding,
    )
    .unwrap();
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    assert_eq!(
        analysis.unchanged_payload_targets(outer_id, after_call, &referent(14)),
        Some(vec![DescriptorLocation::whole(place(13))])
    );
    assert_eq!(
        analysis.unchanged_payload_targets(outer_id, after_call, &referent(15)),
        Some(vec![DescriptorLocation::whole(place(12))])
    );
    drop(analysis);

    // A mutable crossing still admits writes to the old pointed-to slot.
    // Forwarding the replacement descriptor cannot erase that unknown effect.
    module
        .structural_types
        .iter_mut()
        .find(|ty| ty.id == outer_pointer)
        .unwrap()
        .shape = StructuralTypeShape::Reference {
        referent: pointer,
        access: StructuralAccess::MutableBorrow,
    };
    module.boundary_machines[0].reference_outputs[1].access = StructuralAccess::MutableBorrow;
    module.boundary_machines[0].reference_outputs[1].dependencies =
        vec![BoundaryLifetimeDependency::Referent {
            argument_index: 0,
            access: StructuralAccess::MutableBorrow,
        }];
    let graph = super::super::super::super::terminal_graph_with_forwarding(
        &module,
        DescriptorSlotKind::BorrowedStorage,
        &[],
        &module.boundary_reference_forwarding,
    )
    .unwrap();
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    assert_eq!(
        analysis.unchanged_payload_targets(outer_id, after_call, &referent(14)),
        None
    );
}

fn completed_output_module() -> TerminalModule {
    let mut module = module();
    module.structural_types[1].copyable = true;
    if let StructuralTypeShape::Reference { access, .. } = &mut module.structural_types[1].shape {
        *access = StructuralAccess::SharedBorrow;
    }
    for body in &mut module.machines {
        for parameter in &mut body.structural_parameters[..2] {
            parameter.access = StructuralAccess::SharedBorrow;
        }
        for operation in body
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
        {
            match &mut operation.kind {
                OperationKind::EstablishReference { source, .. } => {
                    source.access = StructuralAccess::SharedBorrow
                }
                OperationKind::BoundaryCall {
                    structural_arguments,
                    ..
                } => {
                    for actual in &mut structural_arguments[..2] {
                        actual.access = StructuralAccess::SharedBorrow;
                    }
                }
                _ => {}
            }
        }
    }
    let declaration = &mut module.boundary_machines[0];
    for parameter in &mut declaration.structural_parameters[..2] {
        parameter.access = StructuralAccess::SharedBorrow;
    }
    declaration.reference_outputs[0].access = StructuralAccess::SharedBorrow;
    declaration.reference_outputs[0].dependencies = (0..2)
        .map(|argument_index| BoundaryLifetimeDependency::Referent {
            argument_index,
            access: StructuralAccess::SharedBorrow,
        })
        .collect();
    module
}

fn complete_entry(module: &TerminalModule, body: usize) -> State {
    State {
        values: PayloadContracts(
            module.machines[body]
                .structural_parameters
                .iter()
                .map(|parameter| {
                    (
                        PayloadSubject::Value(parameter.place),
                        Arc::new(
                            terminal_payload_contract(&module.machines[body], parameter.place)
                                .unwrap(),
                        ),
                    )
                })
                .collect(),
        ),
        ..State::default()
    }
}

#[test]
fn completed_forwarding_output_needs_actual_source_and_public_output_rosters_to_agree() {
    let mut module = completed_output_module();
    assert_eq!(
        require(
            &module,
            module.boundary_machines[0].id,
            Some(module.machines[1].id)
        ),
        Ok(())
    );
    let domain = add_source_duty(&mut module);
    let graph = terminal_borrowed_storage_graph(&module);
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    let mut normal = NormalPayloadContext::new(&module, &graph);
    let caller = module.machines[0].id;
    normal.complete_entries.insert(caller);
    let before = complete_entry(&module, 0);
    let outputs = normal
        .completed_call_outputs(
            &mut analysis,
            caller,
            (module.machines[0].entry, 0),
            &before,
        )
        .unwrap();
    assert_eq!(outputs.len(), 1);
    let mut after = before.clone();
    after.withhold_preservation(PlaceId::new(3).unwrap());
    for output in outputs {
        output.publish(&mut after);
    }
    assert_eq!(
        after.preservation_status(PlaceId::new(3).unwrap()),
        "present"
    );
    let mut extra = before;
    extra
        .values
        .get_mut(&PlaceId::new(1).unwrap())
        .unwrap()
        .qualifications
        .push(domain);
    assert_eq!(
        normal
            .completed_call_outputs(&mut analysis, caller, (module.machines[0].entry, 0), &extra)
            .err(),
        Some("the actual source, public source and completed output have different duty rosters")
    );
}

#[test]
fn completed_forwarding_output_cannot_treat_an_unbound_entry_as_a_complete_empty_roster() {
    let module = completed_output_module();
    let graph = terminal_borrowed_storage_graph(&module);
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    let mut normal = NormalPayloadContext::new(&module, &graph);
    assert_eq!(
        normal
            .completed_call_outputs(
                &mut analysis,
                module.machines[0].id,
                (module.machines[0].entry, 0),
                &complete_entry(&module, 0)
            )
            .err(),
        Some("the completed output has no complete actual entry roster")
    );
}

#[test]
fn completed_relay_outputs_are_bound_to_each_actual_call_not_cached_by_body() {
    let mut module = completed_output_module();
    let domain = add_source_duty(&mut module);
    let mut outer = module.machines[0].clone();
    outer.id = MachineId::new(3).unwrap();
    outer.contract = terminal_psi::MachineContract::empty(ContractId::new(3).unwrap());
    outer.blocks[0].operations[0].kind = OperationKind::CallUnit {
        callee: module.machines[0].id,
        arguments: vec![ValueId::new(1).unwrap()],
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: outer
            .structural_parameters
            .iter()
            .map(|parameter| StructuralArgument::whole(parameter.place, parameter.access))
            .collect(),
        requirement_obligations: Vec::new(),
        claim_transfers: Vec::new(),
        crash_continuations: Vec::new(),
    };
    module.machines.push(outer);
    // Copy the completed pointer through a real two-edge join before returning
    // it to the caller. Its block parameter belongs to this invocation too.
    let joined = PlaceId::new(40).unwrap();
    let copied = PlaceId::new(41).unwrap();
    let join = BlockId::new(20).unwrap();
    let reference = StructuralTypeId::new(2).unwrap();
    module.machines[0].structural_places.extend([
        StructuralPlaceDeclaration {
            id: joined,
            kind: StructuralPlaceKind::BlockParameter {
                block: join,
                position: 0,
            },
        },
        StructuralPlaceDeclaration {
            id: copied,
            kind: StructuralPlaceKind::OperationResult {
                producer: OperationId::new(20).unwrap(),
                structural_type: reference,
            },
        },
    ]);
    let edge = |ordinal| SuccessorEdge {
        edge: EdgeId::new(ordinal).unwrap(),
        target: join,
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: vec![StructuralArgument::whole(
            PlaceId::new(3).unwrap(),
            StructuralAccess::Owned,
        )],
        cleanup_actions: Vec::new(),
    };
    module.machines[0].blocks[0].terminator = Terminator::Conditional {
        condition: ValueId::new(1).unwrap(),
        when_true: edge(20),
        when_false: edge(21),
    };
    let mut receiving = terminal_psi::Block::plain(
        join,
        Vec::new(),
        Terminator::ReturnUnit {
            edge: EdgeId::new(22).unwrap(),
            cleanup_actions: Vec::new(),
        },
    );
    let mut arrival = parameter(40, 0, 2);
    arrival.access = StructuralAccess::Owned;
    receiving.structural_parameters.push(arrival);
    receiving.operations.push(Operation::unsuspended(
        OperationId::new(20).unwrap(),
        OperationResult::Structural(StructuralOperationResult {
            place: copied,
            structural_type: reference,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        }),
        OperationKind::StructuralLeafCopy {
            source: joined,
            path: Vec::new(),
        },
    ));
    receiving.operations.push(Operation::unsuspended(
        OperationId::new(21).unwrap(),
        OperationResult::Unit,
        OperationKind::StoreStructuralField {
            disposition: StructuralStoreDisposition::ReplaceDiscardable,
            destination: PlaceId::new(3).unwrap(),
            path: Vec::new(),
            field: None,
            value: StructuralArgument::whole(copied, StructuralAccess::Owned),
        },
    ));
    module.machines[0].blocks.push(receiving);
    let graph = terminal_borrowed_storage_graph(&module);
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    let mut normal = NormalPayloadContext::new(&module, &graph);
    let caller = module.machines[2].id;
    normal.complete_entries.insert(caller);
    let before = complete_entry(&module, 2);
    assert_eq!(
        normal
            .completed_call_outputs(
                &mut analysis,
                caller,
                (module.machines[2].entry, 0),
                &before
            )
            .unwrap()
            .len(),
        1
    );
    let mut other = before.clone();
    other
        .values
        .get_mut(&PlaceId::new(2).unwrap())
        .unwrap()
        .qualifications
        .push(domain);
    assert!(
        normal
            .completed_call_outputs(&mut analysis, caller, (module.machines[2].entry, 0), &other)
            .is_err()
    );
    assert_eq!(
        normal
            .completed_call_outputs(
                &mut analysis,
                caller,
                (module.machines[2].entry, 0),
                &before
            )
            .unwrap()
            .len(),
        1
    );
    let mut cleanup = module.clone();
    let Terminator::ReturnUnit {
        cleanup_actions, ..
    } = &mut cleanup.machines[0].blocks[1].terminator
    else {
        unreachable!()
    };
    cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::InvokeNominal(
        terminal_psi::NominalAffineCleanup {
            retains_descendants: false,
            place: PlaceId::new(3).unwrap(),
            path: Vec::new(),
            structural_type: StructuralTypeId::new(2).unwrap(),
            cleanup_machine: MachineId::new(2).unwrap(),
            consumed_claims: Vec::new(),
            cleanup_receiver: None,
            requirement_obligations: Vec::new(),
        },
    ));
    let graph = terminal_borrowed_storage_graph(&cleanup);
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&cleanup.structural_types, &graph);
    let mut normal = NormalPayloadContext::new(&cleanup, &graph);
    normal.complete_entries.insert(caller);
    assert_eq!(
        normal
            .completed_call_outputs(
                &mut analysis,
                caller,
                (cleanup.machines[2].entry, 0),
                &before
            )
            .err(),
        Some(
            "the normal payload body has an ordered nominal cleanup effect without a replay association"
        )
    );
    let mut graph = terminal_borrowed_storage_graph(&module);
    let descriptor = graph
        .iter_mut()
        .find(|body| body.id == module.machines[0].id)
        .unwrap();
    // A retained unresolved non-call copy must not be rescued by its result
    // annotation, even though all other entries and output proofs are known.
    descriptor.flow.get_mut(&(join, 0)).unwrap().may_escape = true;
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    let mut normal = NormalPayloadContext::new(&module, &graph);
    normal.complete_entries.insert(caller);
    assert_eq!(
        normal
            .completed_call_outputs(
                &mut analysis,
                caller,
                (module.machines[2].entry, 0),
                &before
            )
            .err(),
        Some("the call-bound body has an uncertain non-call value or write effect")
    );
}

#[test]
fn exclusive_saved_pointer_use_retains_all_provider_choices_but_unused_output_does_not() {
    let mut module = completed_output_module();
    module.boundary_reference_forwarding.clear();
    assert!(
        formation_reference_forwarding_requirements(&module)
            .unwrap()
            .is_empty()
    );
    let reference = StructuralTypeId::new(3).unwrap();
    let mut declaration = module.structural_types[1].clone();
    declaration.id = reference;
    declaration.shape = StructuralTypeShape::Reference {
        referent: module.structural_types[1].id,
        access: StructuralAccess::MutableBorrow,
    };
    module.structural_types.push(declaration);
    let operation = OperationId::new(20).unwrap();
    let place = PlaceId::new(20).unwrap();
    module.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::OperationResult {
                producer: operation,
                structural_type: reference,
            },
        });
    module.machines[0].blocks[0]
        .operations
        .push(Operation::unsuspended(
            operation,
            OperationResult::Structural(StructuralOperationResult {
                place,
                structural_type: reference,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            }),
            OperationKind::EstablishReference {
                source: StructuralArgument::whole(
                    PlaceId::new(3).unwrap(),
                    StructuralAccess::MutableBorrow,
                ),
                storage: None,
            },
        ));
    let required = formation_reference_forwarding_requirements(&module).unwrap();
    assert_eq!(required.len(), 1);
    assert_eq!(
        required[0].sources,
        vec![
            BoundaryReferenceSource::Reference { argument_index: 0 },
            BoundaryReferenceSource::Reference { argument_index: 1 }
        ]
    );
    module.boundary_reference_forwarding = required;
    assert_eq!(
        require(
            &module,
            module.boundary_machines[0].id,
            Some(module.machines[1].id)
        ),
        Ok(())
    );
}

#[test]
fn opaque_result_duty_completeness_stays_unknown_through_copy_and_shared_reborrow() {
    let mut module = completed_output_module();
    let domain = add_source_duty(&mut module);
    let reference = StructuralTypeId::new(2).unwrap();
    let mut opaque = module.boundary_machines[0].clone();
    opaque.id = BoundaryMachineId::new(2).unwrap();
    opaque.scalar_parameters.clear();
    opaque.parameter_order = vec![BoundaryParameterKind::Structural];
    opaque.structural_parameters.truncate(1);
    opaque.reference_outputs.clear();
    opaque.normal_qualifications.clear();
    opaque.result =
        BoundaryMachineResult::Structural(terminal_psi::BoundaryStructuralResultDeclaration {
            structural_type: reference,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    opaque.result_reference_regions = vec![terminal_psi::TerminalDynamicReferenceRegion {
        path: Vec::new(),
        origin: terminal_psi::TerminalDynamicReferenceOrigin::Parameter { argument_index: 0 },
        source_path: Vec::new(),
        access: StructuralAccess::SharedBorrow,
    }];
    let private_result = terminal_psi::TerminalMachineResult::Structural(
        terminal_psi::StructuralResultDeclaration {
            place: PlaceId::new(90).unwrap(),
            structural_type: reference,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: vec![StructuralPathQualification {
                path: vec![StructuralPathSegment::Referent],
                domain,
            }],
            reference_sources: Vec::new(),
        },
    );
    assert!(
        opaque.result.accepts_provider_result(
            &private_result,
            opaque.identity,
            None,
            &module.structural_domains
        ),
        "a public result minimum can admit private projected extras"
    );
    let opaque_id = opaque.id;
    module.boundary_machines.push(opaque);
    let result = |place| {
        OperationResult::Structural(StructuralOperationResult {
            place: PlaceId::new(place).unwrap(),
            structural_type: reference,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        })
    };
    let mut install = module.machines[0].blocks[0].operations[0].clone();
    let OperationKind::BoundaryCall {
        structural_arguments,
        ..
    } = &mut install.kind
    else {
        unreachable!()
    };
    structural_arguments[0] = StructuralArgument {
        place: PlaceId::new(22).unwrap(),
        path: vec![StructuralPathSegment::Referent],
        access: StructuralAccess::SharedBorrow,
    };
    module.machines[0].blocks[0].operations = vec![
        Operation::unsuspended(
            OperationId::new(10).unwrap(),
            result(20),
            OperationKind::BoundaryCall {
                boundary: opaque_id,
                arguments: Vec::new(),
                structural_arguments: vec![StructuralArgument::whole(
                    PlaceId::new(1).unwrap(),
                    StructuralAccess::SharedBorrow,
                )],
                completion_receipts: Vec::new(),
                requirement_obligations: Vec::new(),
            },
        ),
        Operation::unsuspended(
            OperationId::new(11).unwrap(),
            result(21),
            OperationKind::StructuralLeafCopy {
                source: PlaceId::new(20).unwrap(),
                path: Vec::new(),
            },
        ),
        Operation::unsuspended(
            OperationId::new(12).unwrap(),
            result(22),
            OperationKind::EstablishReference {
                source: StructuralArgument {
                    place: PlaceId::new(21).unwrap(),
                    path: vec![StructuralPathSegment::Referent],
                    access: StructuralAccess::SharedBorrow,
                },
                storage: None,
            },
        ),
        install,
    ];
    for (place, producer) in [(20, 10), (21, 11), (22, 12)] {
        module.machines[0]
            .structural_places
            .push(StructuralPlaceDeclaration {
                id: PlaceId::new(place).unwrap(),
                kind: StructuralPlaceKind::OperationResult {
                    producer: OperationId::new(producer).unwrap(),
                    structural_type: reference,
                },
            });
    }
    let graph = terminal_borrowed_storage_graph(&module);
    let body = &module.machines[0];
    let position = (body.entry, 3);
    assert!(graph[0].opaque_payload_results.contains(&(body.entry, 0)));
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    let mut history = PayloadMembershipHistory::for_module_preservation(&module, &graph, body);
    for place in [20, 21, 22] {
        assert!(
            history
                .current_payload_contract_in(&mut analysis, position, PlaceId::new(place).unwrap())
                .is_err(),
            "the opaque duty ceiling must remain unknown through every actual value transport"
        );
    }
    let before = &history.replay.cached.as_ref().unwrap().snapshots[&position];
    let mut normal = NormalPayloadContext::new(&module, &graph);
    normal.complete_entries.insert(body.id);
    assert_eq!(
        normal
            .completed_call_outputs(&mut analysis, body.id, position, before)
            .err(),
        Some("the captured actual has no complete current duty roster")
    );

    let mut known = completed_output_module();
    let domain = add_source_duty(&mut known);
    known.machines[0].structural_parameters[0]
        .qualifications
        .push(domain);
    known.boundary_reference_forwarding.clear();
    known.machines[0].blocks[0].operations = vec![Operation::unsuspended(
        OperationId::new(10).unwrap(),
        result(20),
        OperationKind::EstablishReference {
            source: StructuralArgument::whole(
                PlaceId::new(1).unwrap(),
                StructuralAccess::SharedBorrow,
            ),
            storage: None,
        },
    )];
    known.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: PlaceId::new(20).unwrap(),
            kind: StructuralPlaceKind::OperationResult {
                producer: OperationId::new(10).unwrap(),
                structural_type: reference,
            },
        });
    let graph = terminal_borrowed_storage_graph(&known);
    let body = &known.machines[0];
    let mut history = PayloadMembershipHistory::for_module_preservation(&known, &graph, body);
    let contract = history
        .current_payload_contract((body.entry, 1), PlaceId::new(20).unwrap())
        .unwrap();
    assert_eq!(
        contract.projected_qualifications,
        vec![StructuralPathQualification {
            path: vec![StructuralPathSegment::Referent],
            domain
        }],
        "a shared wrapper retains the full known source duties even when its annotation is weaker"
    );
}
