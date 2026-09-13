//! Parameter returns consume actual path-sensitive ownership, not a body template.
use super::*;

fn module() -> TerminalModule {
    let mut module = write_only_primitive_store_module();
    let mut callee = module.machines[0].clone();
    callee.id = machine_id(2);
    callee.entry = block_id(2);
    callee.contract.id = contract_id(2);
    callee.parameters[0].id = value_id(11);
    callee.structural_parameters[0].place = place_id(20);
    callee.structural_places[0].id = place_id(20);
    callee.blocks[0].id = block_id(2);
    callee.blocks[0].operations[0].id = operation_id(11);
    callee.blocks[0].operations[1].id = operation_id(12);
    for operation in &mut callee.blocks[0].operations {
        operation.kind = OperationKind::WriteOnlyPrimitiveStore {
            destination: place_id(20),
            value: value_id(11),
        };
    }
    callee.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(2),
        trivial_affine_discards: Vec::new(),
    };
    module.machines.push(callee);
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(2),
        identity: "ReturnedRecord".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: semantic_vocabulary::StructuralFieldId::new(2).unwrap(),
                identity: "value".into(),
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(signed_i8()),
            }],
        },
    });
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: place_id(2),
            position: 1,
            is_self: false,
            structural_type: structural_type_id(2),
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places.extend([
        StructuralPlaceDeclaration {
            id: place_id(2),
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(3),
            kind: StructuralPlaceKind::Result,
        },
    ]);
    machine.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place: place_id(3),
        structural_type: structural_type_id(2),
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    });
    machine.blocks[0].operations.insert(
        1,
        Operation {
            static_reach_binding: None,
            id: operation_id(3),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                callee: machine_id(2),
                arguments: vec![value_id(1)],
                structural_arguments: vec![StructuralArgument {
                    place: place_id(1),
                    path: Vec::new(),
                    access: StructuralAccess::WriteOnlyBorrow,
                }],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        },
    );
    machine.blocks[0].terminator = Terminator::ReturnStructural {
        edge: edge_id(1),
        source: place_id(2),
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    module
}

fn branch(module: &mut TerminalModule) {
    let machine = &mut module.machines[0];
    machine.parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(2),
        scalar_type: ScalarType::Boolean,
    });
    let mut left = machine.blocks[0].clone();
    left.id = block_id(3);
    left.operations.clear();
    left.terminator = Terminator::ReturnStructural {
        edge: edge_id(5),
        source: place_id(2),
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let mut right = left.clone();
    right.id = block_id(4);
    right.terminator = Terminator::ReturnStructural {
        edge: edge_id(6),
        source: place_id(2),
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let successor = |ordinal| SuccessorEdge {
        edge: edge_id(ordinal),
        target: block_id(ordinal),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    machine.blocks[0].terminator = Terminator::Conditional {
        condition: value_id(2),
        when_true: successor(3),
        when_false: successor(4),
    };
    machine.blocks.extend([left, right]);
}

fn consume(module: &mut TerminalModule, block: usize) {
    let mut callee = module.machines[0].clone();
    callee.id = machine_id(3);
    callee.entry = block_id(5);
    callee.contract.id = contract_id(3);
    callee.parameters.clear();
    callee.structural_parameters = vec![callee.structural_parameters[1].clone()];
    callee.structural_parameters[0].position = 0;
    callee.structural_parameters[0].place = place_id(30);
    callee.structural_places = vec![StructuralPlaceDeclaration {
        id: place_id(30),
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    callee.result = TerminalMachineResult::Unit;
    callee.blocks = vec![Block {
        id: block_id(5),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::ReturnUnit {
            edge: edge_id(7),
            trivial_affine_discards: vec![place_id(30)],
        },
    }];
    module.machines.push(callee);
    module.machines[0].blocks[block].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(4),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: machine_id(3),
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: place_id(2),
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
}

#[test]
fn affine_parameter_return_composes_with_writes_calls_and_multiple_parameters() {
    let module = module();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    let frontiers = reconstruct_structural_ownership_frontiers(&module).unwrap();
    let caller = frontiers.machine(machine_id(1)).unwrap();
    assert!(
        caller
            .operation_exit(operation_id(3))
            .unwrap()
            .owned_places()
            .iter()
            .any(|place| place.place == place_id(2))
    );
    assert!(
        caller
            .edge_entry(edge_id(1))
            .unwrap()
            .owned_places()
            .iter()
            .any(|place| place.place == place_id(2))
    );
}

#[test]
fn unrestricted_record_parameter_return_preserves_exact_owned_custody() {
    let mut original = module();
    original.machines[0].structural_parameters[1].multiplicity =
        StructuralMultiplicity::Unrestricted;
    let TerminalMachineResult::Structural(result) = &mut original.machines[0].result else {
        unreachable!()
    };
    result.multiplicity = StructuralMultiplicity::Unrestricted;
    for branched in [false, true] {
        let mut module = original.clone();
        if branched {
            branch(&mut module);
        }
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .unwrap();
    }
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut changed = original.clone();
        changed.machines[0].structural_parameters[1].access = access;
        assert!(
            validate_module(&changed).is_err(),
            "borrow cannot supply an owned return: {access:?}"
        );
    }
    let mut changed = original.clone();
    changed.machines[0].structural_parameters[1].multiplicity = StructuralMultiplicity::Affine;
    assert!(
        validate_module(&changed).is_err(),
        "affine input cannot become copyable at return"
    );
    let mut changed = original.clone();
    let TerminalMachineResult::Structural(result) = &mut changed.machines[0].result else {
        unreachable!()
    };
    result.structural_type = structural_type_id(1);
    assert!(
        validate_module(&changed).is_err(),
        "return must preserve its exact nominal type"
    );
    let mut changed = original.clone();
    changed
        .structural_domains
        .push(StructuralDomainDeclaration {
            id: domain_id(1),
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(1).unwrap(),
            identity: "QualifiedRecord".into(),
            carrier: structural_type_id(2),
            content_projection: None,
        });
    changed.machines[0].structural_parameters[1]
        .qualifications
        .push(domain_id(1));
    assert!(
        validate_module(&changed).is_err(),
        "return cannot erase source qualifications"
    );
    let mut changed = original;
    let Terminator::ReturnStructural {
        returned_claims, ..
    } = &mut changed.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    returned_claims.push(claim_id(1));
    assert!(
        validate_module(&changed).is_err(),
        "copyable record cannot invent a returned claim"
    );
}

#[test]
fn affine_parameter_return_checks_each_branch() {
    let mut module = module();
    branch(&mut module);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    consume(&mut module, 2);
    assert!(
        matches!(validate_module(&module), Err(ModuleError::StructuralReturnSourceNotLive { machine, block, place }) if machine == machine_id(1) && block == block_id(4) && place == place_id(2))
    );
}

#[test]
fn affine_parameter_return_rejects_prior_consumption() {
    let mut module = module();
    consume(&mut module, 0);
    assert!(
        matches!(validate_module(&module), Err(ModuleError::StructuralReturnSourceNotLive { place, .. }) if place == place_id(2))
    );
}

#[test]
fn affine_parameter_return_rejects_disagreeing_join_custody() {
    let mut module = module();
    branch(&mut module);
    let mut joined = module.machines[0].blocks[1].clone();
    joined.id = block_id(6);
    joined.terminator = Terminator::ReturnStructural {
        edge: edge_id(10),
        source: place_id(2),
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    for (ordinal, block) in module.machines[0].blocks.iter_mut().skip(1).enumerate() {
        block.terminator = Terminator::Jump {
            edge: edge_id(8 + ordinal as u64),
            target: block_id(6),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        };
    }
    module.machines[0].blocks.push(joined);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    consume(&mut module, 2);
    assert!(
        matches!(validate_module(&module), Err(ModuleError::OwnedStructuralFrontierJoinMismatch(block)) if block == block_id(6))
    );
}

#[test]
fn affine_parameter_return_rejects_foreign_sources_borrows_and_changed_contracts() {
    let original = module();
    let mut changed = original.clone();
    let Terminator::ReturnStructural { source, .. } = &mut changed.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    *source = place_id(3);
    assert!(matches!(
        validate_module(&changed),
        Err(ModuleError::StructuralReturnRequiresParameterSource { .. })
    ));
    let mut changed = original.clone();
    changed.machines[0].structural_parameters[1].access = StructuralAccess::SharedBorrow;
    assert!(matches!(
        validate_module(&changed),
        Err(ModuleError::StructuralReturnSourceNotLive { .. })
    ));
    let mut changed = original.clone();
    let TerminalMachineResult::Structural(result) = &mut changed.machines[0].result else {
        unreachable!()
    };
    result.structural_type = structural_type_id(1);
    assert!(matches!(
        validate_module(&changed),
        Err(ModuleError::StructuralReturnSignatureMismatch { .. })
    ));
    let mut changed = original;
    let Terminator::ReturnStructural {
        returned_claims, ..
    } = &mut changed.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    returned_claims.push(claim_id(1));
    assert!(matches!(
        validate_module(&changed),
        Err(ModuleError::StructuralReturnClaimSetMismatch { .. })
    ));
}
