use super::*;
use terminal_verifier::reconstruct_terminal_obligations;

fn term(raw: u64) -> ScalarTerm {
    ScalarTerm::value(value_id(raw), signed_i8())
}

fn store(raw: u64, value: u64) -> Operation {
    Operation {
        static_reach_binding: None,
        id: operation_id(raw),
        result: OperationResult::Unit,
        kind: OperationKind::WriteOnlyPrimitiveStore {
            destination: place_id(1),
            value: value_id(value),
        },
    }
}

fn snapshot_module() -> TerminalModule {
    let mut module = write_only_primitive_store_module();
    let machine = &mut module.machines[0];
    machine.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    machine.parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(2),
        scalar_type: signed_i8(),
    });
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(99),
        scalar_type: signed_i8(),
    });
    machine.blocks[0].operations = vec![
        store(1, 1),
        Operation {
            static_reach_binding: None,
            id: operation_id(2),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(3),
                scalar_type: signed_i8(),
            }),
            kind: OperationKind::PrimitiveScalarRead {
                source: place_id(1),
            },
        },
        store(3, 2),
    ];
    machine.blocks[0].terminator = Terminator::Return {
        edge: edge_id(1),
        value: value_id(3),
        cleanup_actions: Vec::new(),
    };
    machine.contract.ensures = vec![ContractClause {
        obligation: obligation_id(1),
        proposition: Proposition::Equal(term(99), term(1)),
    }];
    module
}

fn snapshot_bundle(module: &TerminalModule) -> ProofBundle {
    let obligations = reconstruct_terminal_obligations(module).expect("valid snapshot module");
    let [obligation] = obligations.obligations() else {
        panic!("one return contract");
    };
    let premise = |left, right| {
        let conclusion = Proposition::Equal(term(left), term(right));
        ProofNode {
            rule: ProofRule::SemanticAxiom {
                index: obligation
                    .semantic_axioms
                    .iter()
                    .position(|fact| fact == &conclusion)
                    .expect("exact reconstructed snapshot premise"),
            },
            conclusion,
        }
    };
    ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).expect("evidence"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: obligation.obligation.proposition.clone(),
                    rule: ProofRule::EqualityTransitivity {
                        left_equals_middle: Box::new(premise(99, 3)),
                        middle_equals_right: Box::new(premise(3, 1)),
                    },
                },
            }),
        }],
        ..ProofBundle::default()
    }
}

fn has_snapshot(module: &TerminalModule) -> bool {
    reconstruct_terminal_obligations(module)
        .expect("valid formation")
        .obligations()[0]
        .semantic_axioms
        .contains(&Proposition::Equal(term(3), term(1)))
}

#[test]
fn primitive_snapshot_survives_later_overwrite_and_stale_proof_rejects_mutations() {
    let module = snapshot_module();
    let bundle = snapshot_bundle(&module);
    verify_module(&module, &bundle, &AdmissionProfile::default()).expect("snapshot proof");

    let mut changed_value = module.clone();
    changed_value.machines[0].blocks[0].operations[0] = store(1, 2);
    assert!(!has_snapshot(&changed_value));
    assert!(verify_module(&changed_value, &bundle, &AdmissionProfile::default()).is_err());

    let mut missing_store = module.clone();
    missing_store.machines[0].blocks[0].operations.remove(0);
    assert!(!has_snapshot(&missing_store));
    assert!(verify_module(&missing_store, &bundle, &AdmissionProfile::default()).is_err());

    let mut wrong_place = module;
    let machine = &mut wrong_place.machines[0];
    let mut second = machine.structural_parameters[0].clone();
    second.place = place_id(2);
    second.position = 1;
    machine.structural_parameters.push(second);
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(2),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    machine.blocks[0].operations[1].kind = OperationKind::PrimitiveScalarRead {
        source: place_id(2),
    };
    assert!(!has_snapshot(&wrong_place));
    assert!(verify_module(&wrong_place, &bundle, &AdmissionProfile::default()).is_err());
}

fn jump(raw: u64, target: u64) -> Terminator {
    Terminator::Jump {
        edge: edge_id(raw),
        target: block_id(target),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    }
}

fn diamond_module() -> TerminalModule {
    let mut module = snapshot_module();
    let machine = &mut module.machines[0];
    machine.parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(4),
        scalar_type: ScalarType::Boolean,
    });
    let tail = machine.blocks[0].operations.split_off(1);
    let terminal = machine.blocks[0].terminator.clone();
    machine.blocks[0].terminator = Terminator::Conditional {
        condition: value_id(4),
        when_true: SuccessorEdge {
            edge: edge_id(2),
            target: block_id(2),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        when_false: SuccessorEdge {
            edge: edge_id(3),
            target: block_id(3),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    for (raw, terminator, operations) in [
        (2, jump(4, 4), Vec::new()),
        (3, jump(5, 4), Vec::new()),
        (4, terminal, tail),
    ] {
        machine.blocks.push(Block {
            id: block_id(raw),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations,
            terminator,
        });
    }
    module
}

#[test]
fn primitive_snapshot_intersects_every_diamond_arrival() {
    let module = diamond_module();
    let bundle = snapshot_bundle(&module);
    verify_module(&module, &bundle, &AdmissionProfile::default()).expect("shared diamond prefix");

    let mut conflict = module.clone();
    conflict.machines[0].blocks[1].operations.push(store(4, 2));
    assert!(!has_snapshot(&conflict));
    assert!(verify_module(&conflict, &bundle, &AdmissionProfile::default()).is_err());

    let mut absent = module;
    absent.machines[0].blocks[0].operations.clear();
    absent.machines[0].blocks[1].operations.push(store(4, 1));
    assert!(!has_snapshot(&absent));
    assert!(verify_module(&absent, &bundle, &AdmissionProfile::default()).is_err());
}

#[test]
fn primitive_snapshot_mutable_call_invalidates_reaching_store_not_captured_value() {
    let mut module = snapshot_module();
    let mut callee = write_only_primitive_store_module().machines.remove(0);
    callee.id = machine_id(2);
    callee.contract.id = contract_id(2);
    callee.parameters[0].id = value_id(10);
    callee.structural_parameters[0].place = place_id(10);
    callee.structural_places[0].id = place_id(10);
    callee.entry = block_id(10);
    callee.blocks[0].id = block_id(10);
    callee.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(10),
        trivial_affine_discards: Vec::new(),
    };
    callee.blocks[0].operations = vec![Operation {
        static_reach_binding: None,
        id: operation_id(10),
        result: OperationResult::Unit,
        kind: OperationKind::WriteOnlyPrimitiveStore {
            destination: place_id(10),
            value: value_id(10),
        },
    }];
    let call = Operation {
        static_reach_binding: None,
        id: operation_id(4),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: callee.id,
            arguments: vec![value_id(2)],
            structural_arguments: vec![StructuralArgument {
                place: place_id(1),
                path: Vec::new(),
                access: StructuralAccess::WriteOnlyBorrow,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    };
    module.machines.push(callee);
    module.machines[0].blocks[0]
        .operations
        .insert(2, call.clone());
    let bundle = snapshot_bundle(&module);
    verify_module(&module, &bundle, &AdmissionProfile::default()).expect("call after capture");
    module.machines[0].blocks[0].operations.remove(2);
    module.machines[0].blocks[0].operations.insert(1, call);
    assert!(!has_snapshot(&module));
    assert!(verify_module(&module, &bundle, &AdmissionProfile::default()).is_err());

    let machine = &mut module.machines[0];
    let mut alias = machine.structural_parameters[0].clone();
    alias.place = place_id(2);
    alias.position = 1;
    machine.structural_parameters.push(alias);
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(2),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut machine.blocks[0].operations[1].kind
    else {
        panic!("mutating call");
    };
    structural_arguments[0].place = place_id(2);
    assert!(
        !has_snapshot(&module),
        "different parameter identities may alias"
    );
    assert!(verify_module(&module, &bundle, &AdmissionProfile::default()).is_err());
}

#[test]
fn primitive_snapshot_requires_cyclic_arrivals_without_losing_iteration_local_stores() {
    let mut module = diamond_module();
    let machine = &mut module.machines[0];
    machine.structural_parameters.clear();
    machine.structural_places[0].kind = StructuralPlaceKind::OperationResult {
        producer: operation_id(1),
        structural_type: structural_type_id(1),
    };
    machine.blocks[0].operations[0] = Operation {
        static_reach_binding: None,
        id: operation_id(1),
        result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
            place: place_id(1),
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishPrimitiveLocal { value: value_id(1) },
    };
    module.machines[0].blocks[2].terminator = Terminator::Conditional {
        condition: value_id(4),
        when_true: SuccessorEdge {
            edge: edge_id(5),
            target: block_id(3),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        when_false: SuccessorEdge {
            edge: edge_id(6),
            target: block_id(4),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    assert!(
        !has_snapshot(&module),
        "prefix store is not an inducted loop invariant"
    );
    module.machines[0].blocks[2].operations.push(store(4, 1));
    let bundle = snapshot_bundle(&module);
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("every exiting iteration explicitly stores the same immutable formal");
}

#[test]
fn primitive_snapshot_fresh_locals_are_disjoint_and_initialization_stays_required() {
    let mut module = snapshot_module();
    let machine = &mut module.machines[0];
    machine.structural_parameters.clear();
    machine.structural_places.clear();
    for (place, producer, value) in [(1, 1, 1), (2, 4, 2)] {
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: place_id(place),
            kind: StructuralPlaceKind::OperationResult {
                producer: operation_id(producer),
                structural_type: structural_type_id(1),
            },
        });
        let establishment = Operation {
            static_reach_binding: None,
            id: operation_id(producer),
            result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                place: place_id(place),
                structural_type: structural_type_id(1),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishPrimitiveLocal {
                value: value_id(value),
            },
        };
        if place == 1 {
            machine.blocks[0].operations[0] = establishment;
        } else {
            machine.blocks[0].operations.insert(1, establishment);
        }
    }
    let bundle = snapshot_bundle(&module);
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("unrelated fresh local does not modify the captured local");
    module.machines[0].blocks[0].operations.remove(0);
    assert!(
        validate_module(&module).is_err(),
        "read equality cannot replace initialization validation"
    );
    assert!(verify_module(&module, &bundle, &AdmissionProfile::default()).is_err());
}
