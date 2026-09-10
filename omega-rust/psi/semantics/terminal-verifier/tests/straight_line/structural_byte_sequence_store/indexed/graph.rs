use super::*;

fn edge(raw: u64, target: u64) -> SuccessorEdge {
    SuccessorEdge {
        edge: id(raw),
        target: id(target),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}

fn jump(raw: u64, target: u64) -> Terminator {
    Terminator::Jump {
        edge: id(raw),
        target: id(target),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        residual_affine_discards: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}

fn split_after_length(module: &mut TerminalModule) {
    let machine = &mut module.machines[0];
    let operations = machine.blocks[0].operations.split_off(5);
    let terminator = std::mem::replace(&mut machine.blocks[0].terminator, jump(901, 901));
    machine.blocks.push(Block {
        id: id(901),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations,
        terminator,
    });
}

fn repeated_replacement(module: &TerminalModule) -> Operation {
    let mut replacement = module.machines[0].blocks[0].operations[3].clone();
    replacement.id = id(8);
    let OperationKind::StructuralByteSequenceFieldStore { obligation, .. } = &mut replacement.kind
    else {
        unreachable!()
    };
    *obligation = id(3);
    replacement
}

#[test]
fn cross_block_observation_is_current_only_without_intervening_replacement() {
    let (mut module, bundle) = indexed_fixture(StructuralAccess::MutableBorrow);
    split_after_length(&mut module);
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    let replacement = repeated_replacement(&module);
    module.machines[0].blocks[1]
        .operations
        .insert(0, replacement);
    assert!(validate_module(&module).is_err());
}

#[test]
fn any_mutating_predecessor_invalidates_the_joined_length() {
    let (mut module, _) = indexed_fixture(StructuralAccess::MutableBorrow);
    split_after_length(&mut module);
    let replacement = repeated_replacement(&module);
    let machine = &mut module.machines[0];
    machine.parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: id(5),
        scalar_type: ScalarType::Boolean,
    });
    machine.blocks[0].terminator = Terminator::Conditional {
        condition: id(5),
        when_true: edge(901, 901),
        when_false: edge(902, 902),
    };
    machine.blocks.push(Block {
        id: id(902),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: vec![replacement],
        terminator: jump(903, 901),
    });
    assert!(validate_module(&module).is_err());
    module.machines[0].blocks[2].operations.clear();
    validate_module(&module).unwrap();
}

#[test]
fn backedge_replacement_invalidates_an_entry_length_but_byte_stores_do_not() {
    let (mut module, _) = indexed_fixture(StructuralAccess::MutableBorrow);
    split_after_length(&mut module);
    let replacement = repeated_replacement(&module);
    let machine = &mut module.machines[0];
    machine.attachment = Some(id(1));
    machine.structural_parameters[0].is_self = true;
    machine.structural_places[0].kind = StructuralPlaceKind::Parameter {
        position: 0,
        is_self: true,
    };
    machine.parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: id(5),
        scalar_type: ScalarType::Boolean,
    });
    let terminator = std::mem::replace(
        &mut machine.blocks[1].terminator,
        Terminator::Conditional {
            condition: id(5),
            when_true: edge(902, 901),
            when_false: edge(903, 902),
        },
    );
    machine.blocks.push(Block {
        id: id(902),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator,
    });
    validate_module(&module).unwrap();
    module.machines[0].blocks[1].operations.push(replacement);
    assert!(
        validate_module(&module).is_err(),
        "the first traversal cannot hide a length-changing backedge"
    );
}

#[test]
fn mutable_call_invalidates_length_but_shared_call_preserves_it() {
    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::SharedBorrow,
    ] {
        let (mut module, _) = indexed_fixture(StructuralAccess::MutableBorrow);
        let mut callee = module.machines[0].clone();
        callee.id = id(901);
        callee.contract.id = id(901);
        callee.entry = id(901);
        callee.structural_parameters[0].place = id(3);
        callee.structural_parameters[0].access = access;
        callee.structural_places = vec![StructuralPlaceDeclaration {
            id: id(3),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }];
        callee.blocks[0].id = id(901);
        callee.blocks[0].operations.clear();
        // Use a fresh machine's exact normal exit shape and globally unique edge.
        if let Terminator::ReturnUnit { edge, .. } = &mut callee.blocks[0].terminator {
            *edge = id(901);
        }
        module.machines.push(callee);
        module.machines[0].blocks[0].operations.insert(
            6,
            Operation {
                id: id(8),
                result: OperationResult::Unit,
                kind: OperationKind::CallUnit {
                    callee: id(901),
                    arguments: Vec::new(),
                    structural_arguments: vec![StructuralArgument {
                        place: id(1),
                        path: Vec::new(),
                        access,
                    }],
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            },
        );
        if access == StructuralAccess::MutableBorrow {
            assert!(validate_module(&module).is_err());
        } else {
            validate_module(&module).unwrap();
        }
    }
}

#[test]
fn repeated_literal_replacement_and_indexed_store_reconstruct_each_iteration() {
    let (mut module, bundle) = indexed_fixture(StructuralAccess::MutableBorrow);
    let machine = &mut module.machines[0];
    machine.attachment = Some(id(1));
    machine.structural_parameters[0].is_self = true;
    machine.structural_places[0].kind = StructuralPlaceKind::Parameter {
        position: 0,
        is_self: true,
    };
    machine.parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: id(5),
        scalar_type: ScalarType::Boolean,
    });
    let operations = std::mem::take(&mut machine.blocks[0].operations);
    let terminator = std::mem::replace(&mut machine.blocks[0].terminator, jump(901, 901));
    machine.blocks.push(Block {
        id: id(901),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations,
        terminator: Terminator::Conditional {
            condition: id(5),
            when_true: edge(902, 901),
            when_false: edge(903, 902),
        },
    });
    machine.blocks.push(Block {
        id: id(902),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator,
    });
    terminal_verifier::verify_module_for_interpretation(
        &module,
        &bundle,
        &AdmissionProfile::default(),
    )
    .unwrap();
    module.machines[0].blocks[1].operations.swap(4, 6);
    assert!(terminal_verifier::validate_module_for_interpretation(&module).is_err());
}

#[test]
fn unknown_live_length_uses_only_the_selected_true_edge_bound() {
    let (mut module, _) = indexed_fixture(StructuralAccess::MutableBorrow);
    let machine = &mut module.machines[0];
    machine.structural_places.retain(|place| place.id == id(1));
    machine.blocks[0].operations.drain(1..4);
    let store = machine.blocks[0].operations.pop().unwrap();
    machine.blocks[0].operations.push(Operation {
        id: id(8),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(5),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::IntegerLessThan {
            left: id(2),
            right: id(3),
        },
    });
    machine.blocks[0].terminator = Terminator::Conditional {
        condition: id(5),
        when_true: edge(901, 901),
        when_false: edge(902, 902),
    };
    for (block, operations) in [(901, vec![store]), (902, Vec::new())] {
        machine.blocks.push(Block {
            id: id(block),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations,
            terminator: Terminator::ReturnUnit {
                edge: id(block + 2),
                trivial_affine_discards: Vec::new(),
            },
        });
    }
    let question = reconstruct_operation_obligations(&module)
        .unwrap()
        .remove(0);
    let length = ScalarTerm::value(id(3), ScalarType::Integer(integer()));
    let index = ScalarTerm::value(id(2), ScalarType::Integer(integer()));
    let bound = Proposition::LessThan(count(0), length);
    let equation = Proposition::Equal(index, count(0));
    let axiom = |conclusion: Proposition| ProofNode {
        rule: ProofRule::SemanticAxiom {
            index: question
                .semantic_axioms
                .iter()
                .position(|candidate| candidate == &conclusion)
                .unwrap(),
        },
        conclusion,
    };
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: question.obligation.id,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: id(2),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: question.obligation.proposition.clone(),
                    rule: ProofRule::IntegerOrderSubstitution {
                        relation: Box::new(axiom(bound.clone())),
                        equality: Box::new(axiom(equation)),
                        endpoint: 0,
                    },
                },
            }),
        }],
        ..ProofBundle::default()
    };
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    std::mem::swap(&mut when_true.target, &mut when_false.target);
    let question = reconstruct_operation_obligations(&module)
        .unwrap()
        .remove(0);
    assert!(!question.semantic_axioms.contains(&bound));
    assert!(verify_module(&module, &bundle, &AdmissionProfile::default()).is_err());
}
