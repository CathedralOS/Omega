use super::super::super::StructuralArgument;
use super::super::{
    Block, StructuralPlaceDeclaration, StructuralPlaceKind, SuccessorEdge, Terminator,
};
use super::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, ObligationEvidence, Operation,
    OperationKind, OperationResult, ProofBundle, ProofNode, ProofRule, ProofSystemMarker,
    Proposition, ScalarTerm, ScalarType, StructuralAccess, TerminalModule, ValueDeclaration, count,
    id, indexed_fixture, integer, reconstruct_operation_obligations, validate_module,
    verify_module,
};
fn edge(raw: u64, target: u64) -> SuccessorEdge {
    SuccessorEdge {
        edge: id(raw),
        target: id(target),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}

fn jump(raw: u64, target: u64) -> Terminator {
    Terminator::Jump {
        edge: id(raw),
        target: id(target),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
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
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
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

fn forwarded_argument(place: u64) -> StructuralArgument {
    StructuralArgument {
        place: id(place),
        path: Vec::new(),
        access: StructuralAccess::MutableBorrow,
    }
}

fn forwarding_jump(raw: u64, target: u64, places: &[u64]) -> Terminator {
    let mut terminator = jump(raw, target);
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut terminator
    else {
        unreachable!()
    };
    *structural_arguments = places.iter().copied().map(forwarded_argument).collect();
    terminator
}

fn parameter_block(
    module: &mut TerminalModule,
    block: u64,
    places: &[u64],
    operations: Vec<Operation>,
    terminator: Terminator,
) -> Block {
    let machine = &mut module.machines[0];
    let structural_parameters = places
        .iter()
        .enumerate()
        .map(|(position, place)| {
            let mut parameter = machine.structural_parameters[0].clone();
            parameter.place = id(*place);
            parameter.position = u32::try_from(position).unwrap();
            machine.structural_places.push(StructuralPlaceDeclaration {
                id: parameter.place,
                kind: StructuralPlaceKind::BlockParameter {
                    block: id(block),
                    position: parameter.position,
                },
            });
            parameter
        })
        .collect();
    Block {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        id: id(block),
        parameters: Vec::new(),
        structural_parameters,
        operations,
        terminator,
    }
}

fn rename_indexed_operations(operations: &mut [Operation], place: u64) {
    for operation in operations {
        match &mut operation.kind {
            OperationKind::StructuralByteSequenceFieldLength { source, .. } => {
                *source = id(place);
            }
            OperationKind::StructuralByteSequenceFieldByteStore { destination, .. } => {
                *destination = id(place);
            }
            _ => {}
        }
    }
}

fn replacement_before_plain_block_and_rename() -> (TerminalModule, ProofBundle) {
    let (mut module, bundle) = indexed_fixture(StructuralAccess::MutableBorrow);
    // The replacement precedes an ordinary edge with no structural bindings.
    // The next edge renames its storage before the length observation.
    let machine = &mut module.machines[0];
    let mut operations = machine.blocks[0].operations.split_off(4);
    rename_indexed_operations(&mut operations, 3);
    let terminator = std::mem::replace(&mut machine.blocks[0].terminator, jump(901, 901));
    let plain = parameter_block(
        &mut module,
        901,
        &[],
        Vec::new(),
        forwarding_jump(902, 902, &[1]),
    );
    let renamed = parameter_block(&mut module, 902, &[3], operations, terminator);
    module.machines[0].blocks.extend([plain, renamed]);
    (module, bundle)
}

fn exact_replacement_length() -> Proposition {
    Proposition::Equal(
        ScalarTerm::value(id(3), ScalarType::Integer(integer())),
        count(3),
    )
}

fn add_other_backing(module: &mut TerminalModule) {
    let machine = &mut module.machines[0];
    let mut parameter = machine.structural_parameters[0].clone();
    parameter.place = id(4);
    parameter.position = 1;
    machine.structural_parameters.push(parameter);
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(4),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
}

#[test]
fn replacement_length_survives_an_empty_structural_telescope_followed_by_a_rename() {
    let (module, bundle) = replacement_before_plain_block_and_rename();
    validate_module(&module).unwrap();
    let questions = reconstruct_operation_obligations(&module).unwrap();
    let byte_store = questions
        .iter()
        .find(|question| question.obligation.id == id(2))
        .unwrap();
    assert!(
        byte_store
            .semantic_axioms
            .contains(&exact_replacement_length())
    );
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
}

#[test]
fn replacement_length_cannot_follow_a_rename_of_different_backing() {
    let (mut module, bundle) = replacement_before_plain_block_and_rename();
    add_other_backing(&mut module);
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[1].terminator
    else {
        unreachable!()
    };
    structural_arguments[0].place = id(4);
    validate_module(&module).unwrap();
    let questions = reconstruct_operation_obligations(&module).unwrap();
    let byte_store = questions
        .iter()
        .find(|question| question.obligation.id == id(2))
        .unwrap();
    assert!(
        !byte_store
            .semantic_axioms
            .contains(&exact_replacement_length())
    );
    assert!(verify_module(&module, &bundle, &AdmissionProfile::default()).is_err());
}

#[test]
fn different_backing_on_one_diamond_arrival_cannot_supply_replacement_length() {
    let (mut module, bundle) = indexed_fixture(StructuralAccess::MutableBorrow);
    add_other_backing(&mut module);
    let machine = &mut module.machines[0];
    machine.parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: id(5),
        scalar_type: ScalarType::Boolean,
    });
    let mut operations = machine.blocks[0].operations.split_off(4);
    rename_indexed_operations(&mut operations, 8);
    let terminator = std::mem::replace(
        &mut machine.blocks[0].terminator,
        forwarding_jump(901, 901, &[1, 4]),
    );
    let mut when_true = edge(902, 902);
    when_true.structural_arguments.push(forwarded_argument(3));
    let mut when_false = edge(903, 903);
    when_false.structural_arguments.push(forwarded_argument(5));
    let branch = parameter_block(
        &mut module,
        901,
        &[3, 5],
        Vec::new(),
        Terminator::Conditional {
            condition: id(5),
            when_true,
            when_false,
        },
    );
    let left = parameter_block(
        &mut module,
        902,
        &[6],
        Vec::new(),
        forwarding_jump(904, 904, &[6]),
    );
    let right = parameter_block(
        &mut module,
        903,
        &[7],
        Vec::new(),
        forwarding_jump(905, 904, &[7]),
    );
    let joined = parameter_block(&mut module, 904, &[8], operations, terminator);
    // This order lets a provisional left-hand name reach the entry before a
    // later traversal discovers the branch's incompatible right-hand name.
    module.machines[0]
        .blocks
        .extend([left, branch, right, joined]);
    for reversed in [false, true] {
        if reversed {
            module.machines[0].blocks[1..].reverse();
        }
        validate_module(&module).unwrap();
        let questions = reconstruct_operation_obligations(&module).unwrap();
        let byte_store = questions
            .iter()
            .find(|question| question.obligation.id == id(2))
            .unwrap();
        assert!(
            !byte_store
                .semantic_axioms
                .contains(&exact_replacement_length()),
            "one arrival denotes untouched backing, regardless of block order ({reversed})"
        );
        assert!(verify_module(&module, &bundle, &AdmissionProfile::default()).is_err());
    }
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
fn sibling_primitive_array_stores_preserve_exact_byte_field_length() {
    use terminal_psi::{
        BindingRelevance, StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
        StructuralTypeDeclaration, StructuralTypeShape,
    };

    for runtime_index in [false, true] {
        for after_observation in [false, true] {
            let (mut module, _) = indexed_fixture(StructuralAccess::MutableBorrow);
            let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape
            else {
                unreachable!()
            };
            fields.push(StructuralFieldDeclaration {
                id: id(2),
                identity: "marked".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Structural(id(4)),
            });
            module.structural_types.extend([
                StructuralTypeDeclaration {
                    id: id(3),
                    identity: "test::Counter".into(),
                    shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer())),
                },
                StructuralTypeDeclaration {
                    id: id(4),
                    identity: "test::Marks".into(),
                    shape: StructuralTypeShape::FixedArray {
                        element: id(3),
                        length: 3,
                    },
                },
            ]);
            let index = if runtime_index {
                StructuralPathSegment::RuntimeIndex {
                    index: id(2),
                    obligation: id(3),
                }
            } else {
                StructuralPathSegment::FixedIndex(0)
            };
            module.machines[0].blocks[0].operations.insert(
                if after_observation { 5 } else { 4 },
                Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id: id(8),
                    result: OperationResult::Unit,
                    kind: OperationKind::WriteOnlyPrimitiveStore {
                        destination: id(1),
                        path: vec![StructuralPathSegment::Field("marked".into()), index],
                        value: id(2),
                    },
                },
            );
            validate_module(&module).unwrap();
            let questions = reconstruct_operation_obligations(&module).unwrap();
            let byte_store = questions
                .iter()
                .find(|question| question.obligation.id == id(2))
                .unwrap();
            assert!(byte_store.semantic_axioms.contains(&Proposition::Equal(
                ScalarTerm::value(id(3), ScalarType::Integer(integer())),
                count(3),
            )));

            // A real replacement of the observed field still invalidates the
            // saved observation, even with the disjoint primitive write present.
            let replacement = repeated_replacement(&module);
            let mut replacement = replacement;
            replacement.id = id(9);
            if let OperationKind::StructuralByteSequenceFieldStore { obligation, .. } =
                &mut replacement.kind
            {
                *obligation = id(4);
            }
            let store_position = module.machines[0].blocks[0].operations.len() - 1;
            module.machines[0].blocks[0]
                .operations
                .insert(store_position, replacement);
            assert!(validate_module(&module).is_err());
        }
    }
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
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
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
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
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
                static_reach_binding: None,
                suspension_crossing: None,
                id: id(8),
                result: OperationResult::Unit,
                kind: OperationKind::CallUnit {
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
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
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
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
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
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
        static_reach_binding: None,
        suspension_crossing: None,
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
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
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
