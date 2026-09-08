use super::*;

fn integer() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
}

fn value(raw: u64) -> ScalarTerm {
    ScalarTerm::value(id(raw, ValueId::new), ScalarType::Integer(integer()))
}

fn literal(raw: u128) -> ScalarTerm {
    ScalarTerm::integer(integer(), IntegerValue::Unsigned(raw)).unwrap()
}

fn scalar_operation(raw: u64, kind: OperationKind) -> Operation {
    Operation {
        id: id(raw, OperationId::new),
        result: OperationResult::Scalar(ValueDeclaration {
            id: id(raw, ValueId::new),
            scalar_type: ScalarType::Integer(integer()),
        }),
        kind,
    }
}

fn argument(raw: u64) -> StructuralArgument {
    StructuralArgument {
        access: StructuralAccess::MutableBorrow,
        ..view_argument(raw)
    }
}

fn fixture() -> TerminalModule {
    let mut module = view_cycle();
    let machine = &mut module.machines[0];
    machine.parameters = vec![ValueDeclaration {
        id: id(30, ValueId::new),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
    }];
    machine.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    machine.blocks[1].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    machine.structural_places[2].kind = StructuralPlaceKind::BlockParameter {
        block: id(3, BlockId::new),
        position: 0,
    };
    let mut store_parameter = machine.blocks[1].structural_parameters[0].clone();
    store_parameter.place = id(3, PlaceId::new);
    machine.blocks[2].structural_parameters = vec![store_parameter];
    for (block, parameter) in [(1, 2), (2, 3)] {
        machine.blocks[block].parameters = vec![ValueDeclaration {
            id: id(parameter, ValueId::new),
            scalar_type: ScalarType::Integer(integer()),
        }];
    }
    machine.blocks[0].operations = vec![scalar_operation(
        1,
        OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(0),
        },
    )];
    machine.blocks[1].operations = vec![
        length_operation(10, 10, 2),
        Operation {
            id: id(11, OperationId::new),
            result: OperationResult::Scalar(ValueDeclaration {
                id: id(11, ValueId::new),
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::IntegerLessThan {
                left: id(2, ValueId::new),
                right: id(10, ValueId::new),
            },
        },
    ];
    let Terminator::Conditional {
        condition,
        when_true,
        ..
    } = &mut machine.blocks[1].terminator
    else {
        unreachable!()
    };
    *condition = id(11, ValueId::new);
    when_true.arguments = vec![id(2, ValueId::new)];
    when_true.structural_arguments = vec![argument(2)];
    machine.blocks[2].operations = vec![
        length_operation(12, 12, 3),
        Operation {
            id: id(13, OperationId::new),
            result: OperationResult::Unit,
            kind: OperationKind::ByteSequenceWrite {
                destination: id(3, PlaceId::new),
                index: id(3, ValueId::new),
                value: id(30, ValueId::new),
                length: id(12, ValueId::new),
                obligation: id(1, ObligationId::new),
            },
        },
        scalar_operation(
            14,
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(1),
            },
        ),
        scalar_operation(
            15,
            OperationKind::ExactIntegerAdd {
                left: id(3, ValueId::new),
                right: id(14, ValueId::new),
                obligation: id(2, ObligationId::new),
            },
        ),
    ];
    for (block, source, scalar) in [(0, 1, 1), (2, 3, 15)] {
        let Terminator::Jump {
            arguments,
            structural_arguments,
            ..
        } = &mut machine.blocks[block].terminator
        else {
            unreachable!()
        };
        *arguments = vec![id(scalar, ValueId::new)];
        *structural_arguments = vec![argument(source)];
    }
    module
}

fn axiom(axioms: &[Proposition], conclusion: Proposition) -> ProofNode {
    let index = axioms
        .iter()
        .position(|candidate| *candidate == conclusion)
        .unwrap_or_else(|| panic!("missing exact axiom {conclusion:?} in {axioms:#?}"));
    ProofNode {
        conclusion,
        rule: ProofRule::SemanticAxiom { index },
    }
}

fn proof(module: &TerminalModule) -> ProofBundle {
    let obligations = reconstruct_interpretable_operation_obligations(
        validate_module_for_interpretation(module).expect("fresh mutable write cycle validates"),
    )
    .unwrap();
    assert_eq!(obligations.len(), 2);
    let bounds = &obligations[0];
    let write = ProofNode {
        conclusion: bounds.obligation.proposition.clone(),
        rule: ProofRule::IntegerOrderSubstitution {
            relation: Box::new(axiom(
                &bounds.semantic_axioms,
                Proposition::LessThan(value(3), value(10)),
            )),
            equality: Box::new(axiom(
                &bounds.semantic_axioms,
                Proposition::Equal(value(12), value(10)),
            )),
            endpoint: 1,
        },
    };
    let increment = &obligations[1];
    let upper = axiom(
        &increment.semantic_axioms,
        Proposition::LessOrEqual(value(3), literal(u128::from(u64::MAX) - 1)),
    );
    let one = axiom(
        &increment.semantic_axioms,
        Proposition::Equal(value(14), literal(1)),
    );
    let root_bound = ProofNode {
        conclusion: Proposition::Conjunction(vec![
            upper.conclusion.clone(),
            one.conclusion.clone(),
        ]),
        rule: ProofRule::ConjunctionIntroduction(vec![upper, one]),
    };
    let addition = ProofNode {
        conclusion: increment.obligation.proposition.clone(),
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(root_bound),
            witness: IntegerAffineWitness {
                root: value(3),
                target: ScalarTerm::exact_integer_add(integer(), value(3), value(14)).unwrap(),
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            },
        },
    };
    ProofBundle {
        evidence: [write, addition]
            .into_iter()
            .zip(obligations)
            .enumerate()
            .map(|(position, (proof, reconstructed))| ObligationEvidence {
                obligation: reconstructed.obligation.id,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: id(position as u64 + 1, EvidenceIdentity::new),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof,
                }),
            })
            .collect(),
        ..ProofBundle::default()
    }
}

#[test]
fn fresh_guard_write_cycle_replays_bounds_and_exact_increment_certificates() {
    let module = fixture();
    let bundle = proof(&module);
    verify_module_for_interpretation(&module, &bundle, &AdmissionProfile::default()).unwrap();
    let mut reordered = module.clone();
    reordered.machines[0].blocks.reverse();
    verify_module_for_interpretation(&reordered, &bundle, &AdmissionProfile::default()).unwrap();
    assert!(module.machines[0].ranked_scc.is_none());
    assert!(
        verify_module_for_interpretation(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err()
    );
}

#[test]
fn fresh_guard_write_cycle_rejects_changed_guard_index_extent_and_mutable_custody() {
    let module = fixture();
    let bundle = proof(&module);
    for mutation in 0..6 {
        let mut changed = module.clone();
        let machine = &mut changed.machines[0];
        match mutation {
            0 => {
                machine.blocks[1].operations[1].kind = OperationKind::IntegerLessOrEqual {
                    left: id(2, ValueId::new),
                    right: id(10, ValueId::new),
                }
            }
            1 => {
                let OperationKind::ByteSequenceWrite { index, .. } =
                    &mut machine.blocks[2].operations[1].kind
                else {
                    unreachable!()
                };
                *index = id(10, ValueId::new);
            }
            2 => {
                let OperationKind::ByteSequenceWrite { length, .. } =
                    &mut machine.blocks[2].operations[1].kind
                else {
                    unreachable!()
                };
                *length = id(10, ValueId::new);
            }
            3 => {
                let Terminator::Jump {
                    structural_arguments,
                    ..
                } = &mut machine.blocks[2].terminator
                else {
                    unreachable!()
                };
                structural_arguments[0] = argument(1);
            }
            4 => machine.blocks[2].structural_parameters[0].access = StructuralAccess::SharedBorrow,
            _ => {
                let Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } = &mut machine.blocks[1].terminator
                else {
                    unreachable!()
                };
                std::mem::swap(when_true, when_false);
            }
        }
        assert!(
            verify_module_for_interpretation(&changed, &bundle, &AdmissionProfile::default())
                .is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn fresh_guard_write_cycle_cannot_duplicate_an_exclusive_backedge_binding() {
    let mut module = fixture();
    let machine = &mut module.machines[0];
    let mut second = machine.structural_parameters[0].clone();
    second.position = 1;
    second.place = id(4, PlaceId::new);
    machine.structural_parameters.push(second.clone());
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: second.place,
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    for (block, place) in [(1, 5), (2, 6)] {
        second.place = id(place, PlaceId::new);
        machine.blocks[block]
            .structural_parameters
            .push(second.clone());
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: second.place,
            kind: StructuralPlaceKind::BlockParameter {
                block: machine.blocks[block].id,
                position: 1,
            },
        });
    }
    for (block, place) in [(0, 4), (2, 6)] {
        let Terminator::Jump {
            structural_arguments,
            ..
        } = &mut machine.blocks[block].terminator
        else {
            unreachable!()
        };
        structural_arguments.push(argument(place));
    }
    let Terminator::Conditional { when_true, .. } = &mut machine.blocks[1].terminator else {
        unreachable!()
    };
    when_true.structural_arguments.push(argument(5));
    let bundle = proof(&module);
    verify_module_for_interpretation(&module, &bundle, &AdmissionProfile::default()).unwrap();
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[2].terminator
    else {
        unreachable!()
    };
    structural_arguments[1] = argument(3);
    assert!(
        verify_module_for_interpretation(&module, &bundle, &AdmissionProfile::default()).is_err()
    );
}
