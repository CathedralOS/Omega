use super::{
    AdmissionProfile, Block, BlockId, BoundaryMachineId, ContractId, EdgeId, IntegerSign,
    IntegerType, IntegerValue, MachineId, ModuleError, Operation, OperationId, OperationKind,
    OperationResult, ProofBundle, ScalarType, StructuralTypeId, SuccessorEdge, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, ValueId, unit_module,
    validate_module, verify_module,
};
use semantic_vocabulary::{DomainSemanticId, ScalarDomainId, ScalarQualificationSetId};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, MachineContract,
    ProviderCandidateConformance, ProviderRefinement, ProviderSignature, ScalarDomainDeclaration,
    ScalarDomainEstablishmentRoute, ScalarQualificationCoercion, ScalarQualificationSet,
    StructuralTypeDeclaration, StructuralTypeShape,
};

fn value(raw: u64, set: u64) -> ValueDeclaration {
    ValueDeclaration {
        id: ValueId::new(raw).unwrap(),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        qualifications: ScalarQualificationSetId::new(set),
    }
}

fn module() -> TerminalModule {
    let mut module = unit_module();
    let machine = &mut module.machines[0];
    machine.parameters = vec![value(1, 0)];
    machine.result = TerminalMachineResult::Scalar(value(4, 1));
    machine.blocks[0].terminator = Terminator::Jump {
        edge: EdgeId::new(1).unwrap(),
        target: BlockId::new(2).unwrap(),
        arguments: vec![value(1, 0).id],
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: vec![],
        trivial_affine_discards: vec![],
        residual_affine_discards: vec![],
    };
    machine.blocks.push(Block {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        id: BlockId::new(2).unwrap(),
        parameters: vec![value(2, 1)],
        structural_parameters: vec![],
        operations: vec![],
        terminator: Terminator::Return {
            edge: EdgeId::new(2).unwrap(),
            value: value(2, 1).id,
            cleanup_actions: vec![],
        },
    });
    module.scalar_qualifications.domains = vec![ScalarDomainDeclaration {
        id: ScalarDomainId::new(1).unwrap(),
        semantic_domain: DomainSemanticId::new(1).unwrap(),
        identity: "test::Km<u64>".into(),
        carrier: value(1, 0).scalar_type,
        establishment_routes: Vec::new(),
    }];
    module.scalar_qualifications.sets = vec![ScalarQualificationSet {
        id: ScalarQualificationSetId::new(1),
        domains: vec![ScalarDomainId::new(1).unwrap()],
    }];
    module.scalar_qualifications.coercions = vec![ScalarQualificationCoercion {
        machine: machine.id,
        edge: EdgeId::new(1).unwrap(),
        argument_ordinal: 0,
        source: value(1, 0).id,
        destination: value(2, 1).id,
    }];
    module
}

#[test]
fn scalar_membership_introduction_and_return_verify_under_every_policy() {
    let module = module();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    terminal_verifier::validate_module_for_interpretation(&module).unwrap();
    terminal_verifier::validate_module_for_optimization(&module).unwrap();
    terminal_verifier::validate_module_representation(&module).unwrap();
}

fn erasure_module() -> TerminalModule {
    let mut module = module();
    module.machines[0].parameters[0].qualifications = ScalarQualificationSetId::new(1);
    module.machines[0].blocks[1].parameters[0].qualifications = ScalarQualificationSetId::ZERO;
    module.machines[0]
        .result
        .scalar_mut()
        .unwrap()
        .qualifications = ScalarQualificationSetId::ZERO;
    module
}

#[test]
fn scalar_membership_explicit_erasure_verifies_under_every_policy() {
    let module = erasure_module();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    terminal_verifier::validate_module_for_interpretation(&module).unwrap();
    terminal_verifier::validate_module_for_optimization(&module).unwrap();
    terminal_verifier::validate_module_representation(&module).unwrap();
}

#[test]
fn scalar_membership_erasure_rejects_implicit_stale_equal_and_orphan_edges() {
    let changes: &[fn(&mut TerminalModule)] = &[
        |module| module.scalar_qualifications.coercions.clear(),
        |module| {
            module
                .scalar_qualifications
                .coercions
                .push(module.scalar_qualifications.coercions[0])
        },
        |module| module.scalar_qualifications.coercions[0].edge = EdgeId::new(2).unwrap(),
        |module| module.scalar_qualifications.coercions[0].argument_ordinal = 1,
        |module| module.scalar_qualifications.coercions[0].source = value(2, 0).id,
        |module| module.scalar_qualifications.coercions[0].destination = value(4, 0).id,
        |module| module.scalar_qualifications.coercions[0].machine = MachineId::new(999).unwrap(),
        |module| module.machines[0].parameters[0].qualifications = ScalarQualificationSetId::ZERO,
        |module| module.machines[0].parameters[0].scalar_type = ScalarType::Boolean,
    ];
    for (index, change) in changes.iter().enumerate() {
        let mut module = erasure_module();
        change(&mut module);
        assert!(
            validate_module(&module).is_err(),
            "erasure mutation {index}"
        );
    }
}

#[test]
fn scalar_membership_partial_erasure_preserves_remaining_atoms_and_rejects_replacement() {
    let mut module = module();
    let mut second = module.scalar_qualifications.domains[0].clone();
    second.id = ScalarDomainId::new(2).unwrap();
    second.semantic_domain = DomainSemanticId::new(2).unwrap();
    second.identity = "test::Other<u64>".into();
    module.scalar_qualifications.domains.push(second);
    module
        .scalar_qualifications
        .sets
        .push(ScalarQualificationSet {
            id: ScalarQualificationSetId::new(2),
            domains: vec![
                ScalarDomainId::new(1).unwrap(),
                ScalarDomainId::new(2).unwrap(),
            ],
        });
    module.machines[0].parameters[0].qualifications = ScalarQualificationSetId::new(2);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    // Replacing Other by Km is neither strict addition nor strict removal.
    module.scalar_qualifications.sets[1].domains.remove(0);
    assert!(validate_module(&module).is_err());
}

#[test]
fn observing_a_qualified_boolean_preserves_its_membership() {
    let mut module = module();
    module.scalar_qualifications.domains[0].carrier = ScalarType::Boolean;
    let machine = &mut module.machines[0];
    machine.parameters[0].scalar_type = ScalarType::Boolean;
    machine.result.scalar_mut().unwrap().scalar_type = ScalarType::Boolean;
    machine.blocks[1].parameters[0].scalar_type = ScalarType::Boolean;
    let result = machine.blocks[1].parameters[0];
    let successor = |edge| terminal_psi::SuccessorEdge {
        edge: EdgeId::new(edge).unwrap(),
        target: BlockId::new(3).unwrap(),
        arguments: vec![result.id],
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: vec![],
        trivial_affine_discards: vec![],
    };
    machine.blocks[1].terminator = Terminator::Conditional {
        condition: result.id,
        when_true: successor(2),
        when_false: successor(3),
    };
    let mut destination = result;
    destination.id = ValueId::new(5).unwrap();
    machine.blocks.push(Block {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        id: BlockId::new(3).unwrap(),
        parameters: vec![destination],
        structural_parameters: vec![],
        operations: vec![],
        terminator: Terminator::Return {
            edge: EdgeId::new(4).unwrap(),
            value: destination.id,
            cleanup_actions: vec![],
        },
    });
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
}

#[test]
fn scalar_membership_rejects_missing_or_redirected_coercion_and_definitions() {
    let changes: &[fn(&mut TerminalModule)] = &[
        |module| module.scalar_qualifications.coercions.clear(),
        |module| {
            let row = module.scalar_qualifications.coercions[0];
            module.scalar_qualifications.coercions.push(row);
        },
        |module| module.scalar_qualifications.coercions[0].edge = EdgeId::new(2).unwrap(),
        |module| module.scalar_qualifications.coercions[0].argument_ordinal = 1,
        |module| module.scalar_qualifications.coercions[0].source = value(2, 1).id,
        |module| module.scalar_qualifications.coercions[0].destination = value(4, 1).id,
        |module| module.scalar_qualifications.coercions[0].machine = MachineId::new(999).unwrap(),
        |module| module.scalar_qualifications.sets.clear(),
        |module| module.scalar_qualifications.domains.clear(),
        |module| {
            module.scalar_qualifications.sets[0]
                .domains
                .push(ScalarDomainId::new(1).unwrap())
        },
        |module| module.scalar_qualifications.sets[0].domains[0] = ScalarDomainId::new(2).unwrap(),
        |module| module.scalar_qualifications.domains[0].carrier = ScalarType::Boolean,
        |module| {
            let row = module.scalar_qualifications.domains[0].clone();
            module.scalar_qualifications.domains.push(row);
        },
        |module| {
            let mut row = module.scalar_qualifications.domains[0].clone();
            row.id = ScalarDomainId::new(2).unwrap();
            module.scalar_qualifications.domains.push(row);
        },
        |module| {
            let mut row = module.scalar_qualifications.sets[0].clone();
            row.id = ScalarQualificationSetId::new(2);
            module.scalar_qualifications.sets.push(row);
        },
        |module| {
            module.machines[0]
                .result
                .scalar_mut()
                .unwrap()
                .qualifications = ScalarQualificationSetId::new(0)
        },
        |module| module.machines[0].parameters[0].qualifications = ScalarQualificationSetId::new(1),
        |module| {
            if let Terminator::Jump { arguments, .. } = &mut module.machines[0].blocks[0].terminator
            {
                arguments.clear();
            }
        },
    ];
    for (index, change) in changes.iter().enumerate() {
        let mut module = module();
        change(&mut module);
        assert!(validate_module(&module).is_err(), "mutation {index}");
    }
}

#[test]
fn scalar_membership_cannot_be_forged_on_an_operation_or_consumed_by_arithmetic() {
    let mut forged = module();
    forged.machines[0].blocks[1].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Scalar(value(5, 1)),
        kind: OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(7),
        },
    });
    assert!(validate_module(&forged).is_err());
    forged.machines[0].blocks[1].operations[0] = Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Scalar(value(5, 0)),
        kind: OperationKind::IntegerBitwiseNot {
            operand: value(2, 1).id,
        },
    };
    assert!(validate_module(&forged).is_err());
}

#[test]
fn scalar_domain_establishment_routes_require_a_retained_issuer() {
    let mut module = module();
    module.scalar_qualifications.domains[0].establishment_routes =
        vec![ScalarDomainEstablishmentRoute::CheckedRequirement {
            requirement_identity: "test::Requirement<1>".into(),
        }];
    // No boundary, provider, conformance, or dispatch row carries the named
    // requirement: the forged issuer has nothing to resolve against.
    assert!(validate_module(&module).is_err());
    module.scalar_qualifications.domains[0].establishment_routes = vec![
        ScalarDomainEstablishmentRoute::ExactMachine {
            machine_identity: "test::issuer".into(),
        },
        ScalarDomainEstablishmentRoute::CheckedRequirement {
            requirement_identity: "test::Requirement<1>".into(),
        },
    ];
    assert!(validate_module(&module).is_err());
    module.scalar_qualifications.domains[0].establishment_routes =
        vec![ScalarDomainEstablishmentRoute::CheckedRequirement {
            requirement_identity: String::new(),
        }];
    assert!(validate_module(&module).is_err());
}

// A boundary requirement, its provider candidate, and a second machine
// carrying its own introduction coercion: every issuer route for the
// fixture domain resolves to a machine this module binds — never to the
// fixture machine that also introduces the domain.
fn routed_issuer(module: &mut TerminalModule) -> MachineId {
    module.scalar_qualifications.domains[0].establishment_routes = vec![
        ScalarDomainEstablishmentRoute::CheckedRequirement {
            requirement_identity: "test::observe".into(),
        },
        ScalarDomainEstablishmentRoute::ExactMachine {
            machine_identity: "test::issuer".into(),
        },
    ];
    module.boundary_machines.push(BoundaryMachineDeclaration {
        id: BoundaryMachineId::new(1).unwrap(),
        identity: "test::observe".into(),
        attachment: None,
        parameter_order: vec![terminal_psi::BoundaryParameterKind::Scalar],
        scalar_parameters: vec![value(951, 0).scalar_type],
        crash_routes: Vec::new(),
        structural_parameters: Vec::new(),
        result: BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        fixed_service_reach: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    module.structural_types.push(StructuralTypeDeclaration {
        id: StructuralTypeId::new(1).unwrap(),
        identity: "test::Issuer".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let issuer = MachineId::new(950).unwrap();
    module.machines.push(TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: issuer,
        attachment: Some(StructuralTypeId::new(1).unwrap()),
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: vec![value(951, 0)],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(951).unwrap(),
        blocks: vec![
            Block {
                erased_proof_formals: Vec::new(),
                erased_scalar_formals: Vec::new(),
                id: BlockId::new(951).unwrap(),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Jump {
                    erased_proof_arguments: Vec::new(),
                    edge: EdgeId::new(951).unwrap(),
                    target: BlockId::new(952).unwrap(),
                    arguments: vec![value(951, 0).id],
                    erased_arguments: Vec::new(),
                    structural_arguments: vec![],
                    trivial_affine_discards: vec![],
                    residual_affine_discards: vec![],
                },
            },
            Block {
                erased_proof_formals: Vec::new(),
                erased_scalar_formals: Vec::new(),
                id: BlockId::new(952).unwrap(),
                parameters: vec![value(952, 1)],
                structural_parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: EdgeId::new(952).unwrap(),
                    trivial_affine_discards: vec![],
                },
            },
        ],
        contract: MachineContract {
            erased_proof_formals: Vec::new(),
            erased_scalar_formals: Vec::new(),
            id: ContractId::new(950).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    });
    module
        .provider_candidates
        .push(ProviderCandidateConformance {
            boundary: BoundaryMachineId::new(1).unwrap(),
            requirement_identity: "test::observe".into(),
            provider_identity: "test::Issuer::observe".into(),
            candidate_identity: "test::issuer".into(),
            candidate: issuer,
            signature: ProviderSignature {
                parameters: Vec::new(),
            },
            refinement: ProviderRefinement {
                positional_parameters: Vec::new(),
                required_domains: Vec::new(),
                realized_service_ceiling: Vec::new(),
            },
        });
    module
        .scalar_qualifications
        .coercions
        .push(ScalarQualificationCoercion {
            machine: issuer,
            edge: EdgeId::new(951).unwrap(),
            argument_ordinal: 0,
            source: value(951, 0).id,
            destination: value(952, 1).id,
        });
    issuer
}

#[test]
fn routed_domain_introduction_requires_an_issuer_bound_to_the_machine() {
    let mut module = module();
    let issuer = routed_issuer(&mut module);
    // The fixture machine's own introduction edge now gains a routed domain
    // while binding to no issuer row — a caller cannot mint membership on the
    // provider's authority.
    let outcome = validate_module(&module);
    assert!(
        matches!(
            outcome,
            Err(ModuleError::InvalidScalarQualification(reason))
                if reason
                    == "scalar qualification introduction has no issuer route bound to this machine"
        ),
        "unexpected outcome: {outcome:?}"
    );
    // The bound issuer machine's own introduction edge admits both routes.
    module
        .scalar_qualifications
        .coercions
        .retain(|coercion| coercion.machine == issuer);
    module.machines[0].parameters[0].qualifications = ScalarQualificationSetId::new(0);
    module.machines[0].blocks[1].parameters[0].qualifications = ScalarQualificationSetId::new(0);
    module.machines[0]
        .result
        .scalar_mut()
        .unwrap()
        .qualifications = ScalarQualificationSetId::new(0);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    // Erasure is not establishment: dropping the routed domain's membership
    // needs no issuer authority.
    let mut erased = erasure_module();
    erased.scalar_qualifications.domains[0].establishment_routes =
        module.scalar_qualifications.domains[0]
            .establishment_routes
            .clone();
    erased.structural_types.clone_from(&module.structural_types);
    erased
        .boundary_machines
        .clone_from(&module.boundary_machines);
    erased
        .provider_candidates
        .clone_from(&module.provider_candidates);
    erased.machines.push(module.machines[1].clone());
    erased
        .scalar_qualifications
        .coercions
        .extend(module.scalar_qualifications.coercions.iter().cloned());
    verify_module(
        &erased,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
}

#[test]
fn scalar_membership_calls_transport_both_arguments_and_results() {
    let mut module = module();
    let mut callee = unit_module().machines.remove(0);
    callee.id = MachineId::new(901).unwrap();
    callee.contract.id = ContractId::new(901).unwrap();
    callee.entry = BlockId::new(901).unwrap();
    callee.parameters = vec![value(10, 1)];
    callee.result = TerminalMachineResult::Scalar(value(11, 1));
    callee.blocks[0].id = callee.entry;
    callee.blocks[0].terminator = Terminator::Return {
        edge: EdgeId::new(901).unwrap(),
        value: value(10, 1).id,
        cleanup_actions: vec![],
    };
    module.machines[0].blocks[1].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Scalar(value(3, 1)),
        kind: OperationKind::Call {
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            callee: callee.id,
            arguments: vec![value(2, 1).id],
            requirement_obligations: vec![],
            crash_continuations: vec![],
        },
    });
    module.machines[0].blocks[1].terminator = Terminator::Return {
        edge: EdgeId::new(2).unwrap(),
        value: value(3, 1).id,
        cleanup_actions: vec![],
    };
    module.machines.push(callee);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    let mut changed = module.clone();
    changed.machines[0].blocks[1].operations[0]
        .result
        .scalar_mut()
        .unwrap()
        .qualifications = ScalarQualificationSetId::new(0);
    assert!(validate_module(&changed).is_err());
    if let OperationKind::Call { arguments, .. } =
        &mut module.machines[0].blocks[1].operations[0].kind
    {
        arguments[0] = value(1, 0).id;
    }
    assert!(validate_module(&module).is_err());
}

#[test]
fn scalar_membership_join_requires_qualification_on_every_arrival() {
    let mut module = module();
    let machine = &mut module.machines[0];
    machine.parameters.push(ValueDeclaration {
        id: ValueId::new(20).unwrap(),
        scalar_type: ScalarType::Boolean,
        qualifications: ScalarQualificationSetId::new(0),
    });
    machine.blocks[0].terminator = Terminator::Conditional {
        condition: ValueId::new(20).unwrap(),
        when_true: SuccessorEdge {
            edge: EdgeId::new(1).unwrap(),
            target: BlockId::new(2).unwrap(),
            arguments: vec![value(1, 0).id],
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: vec![],
            trivial_affine_discards: vec![],
        },
        when_false: SuccessorEdge {
            edge: EdgeId::new(3).unwrap(),
            target: BlockId::new(2).unwrap(),
            arguments: vec![value(1, 0).id],
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: vec![],
            trivial_affine_discards: vec![],
        },
    };
    let mut second = module.scalar_qualifications.coercions[0];
    second.edge = EdgeId::new(3).unwrap();
    module.scalar_qualifications.coercions.push(second);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    module.scalar_qualifications.coercions.pop();
    assert!(validate_module(&module).is_err());
}
