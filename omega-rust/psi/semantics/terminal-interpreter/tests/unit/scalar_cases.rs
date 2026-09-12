use super::*;

fn count_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
}

fn count(value: u64) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: count_type(),
        value: IntegerValue::Unsigned(value.into()),
    }
}

#[test]
fn scalar_case_constructor_mixed_call_requires_exact_positional_proof() {
    let mut module = scalar_case_call_module();
    let term = |identity| ScalarTerm::value(value_id(identity), ScalarType::Integer(count_type()));
    let caller_requirement = Proposition::LessOrEqual(term(10), term(11));
    module.machines[0].contract.requires = vec![caller_requirement.clone()];
    module.machines[1].contract.requires = vec![Proposition::LessOrEqual(term(20), term(21))];
    let OperationKind::CallStructuralWithScalarArguments {
        requirement_obligations,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *requirement_obligations = vec![obligation_id(1)];
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: caller_requirement.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                },
            }),
        }],
        ..ProofBundle::default()
    };
    let reconstructed = terminal_verifier::reconstruct_terminal_obligations(&module).unwrap();
    let site = reconstructed
        .obligations()
        .iter()
        .find(|site| site.obligation.id == obligation_id(1))
        .unwrap();
    assert_eq!(
        site.owner,
        terminal_verifier::ReconstructedTerminalObligationOwner::CallRequires {
            machine: machine_id(1),
            operation: operation_id(2),
            requirement_position: 0,
        }
    );
    assert_eq!(site.obligation.proposition, caller_requirement);
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    assert!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err()
    );
    let mut forged = module.clone();
    let OperationKind::CallStructuralWithScalarArguments { arguments, .. } =
        &mut forged.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    arguments.swap(0, 1);
    assert!(
        verify_module(&forged, &bundle, &AdmissionProfile::default()).is_err(),
        "same-typed actuals retain positional meaning"
    );
    let measured = interpret_terminal_artifact_measured(
        &encode_module(&module).unwrap(),
        &encode_proof_bundle(&bundle).unwrap(),
        &AdmissionProfile::default(),
        &[count(17), count(83)],
    )
    .unwrap();
    assert_eq!(measured.value(), TerminalExecutionResult::Scalar(count(17)));
}

#[test]
fn scalar_case_constructor_call_accepts_only_available_exact_borrowed_block_views() {
    let mut module = scalar_case_call_module();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(2),
        identity: "Bytes".into(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
    });
    let parameter = |place| StructuralParameterDeclaration {
        place: place_id(place),
        position: 0,
        is_self: false,
        structural_type: structural_type_id(2),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: vec![],
        projected_qualifications: vec![],
    };
    module.machines[1].structural_parameters = vec![parameter(6)];
    module.machines[1]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: place_id(6),
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        });
    let caller = &mut module.machines[0];
    caller.structural_parameters = vec![parameter(5)];
    caller.structural_places.extend([
        StructuralPlaceDeclaration {
            id: place_id(5),
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(7),
            kind: semantic_vocabulary::StructuralPlaceKind::BlockParameter {
                block: block_id(4),
                position: 0,
            },
        },
    ]);
    let mut call_block = caller.blocks.remove(0);
    call_block.id = block_id(4);
    call_block.structural_parameters = vec![parameter(7)];
    let OperationKind::CallStructuralWithScalarArguments {
        structural_arguments,
        ..
    } = &mut call_block.operations[0].kind
    else {
        unreachable!()
    };
    *structural_arguments = vec![StructuralArgument {
        place: place_id(7),
        path: vec![],
        access: StructuralAccess::MutableBorrow,
    }];
    caller.blocks.insert(
        0,
        Block {
            id: block_id(1),
            parameters: vec![],
            structural_parameters: vec![],
            operations: vec![],
            terminator: Terminator::Jump {
                edge: edge_id(4),
                target: block_id(4),
                arguments: vec![],
                structural_arguments: vec![StructuralArgument {
                    place: place_id(5),
                    path: vec![],
                    access: StructuralAccess::MutableBorrow,
                }],
                trivial_affine_discards: vec![],
                residual_affine_discards: vec![],
            },
        },
    );
    caller.blocks.push(call_block);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    for corruption in 0..3 {
        let mut forged = module.clone();
        let caller = &mut forged.machines[0];
        match corruption {
            0 => {
                let OperationKind::CallStructuralWithScalarArguments {
                    structural_arguments,
                    ..
                } = &mut caller.blocks[2].operations[0].kind
                else {
                    unreachable!()
                };
                structural_arguments[0].access = StructuralAccess::SharedBorrow;
            }
            1 => {
                let call = caller.blocks[2].operations.remove(0);
                caller.blocks[0].operations.push(call);
            }
            2 => caller.blocks[2].structural_parameters[0].structural_type = structural_type_id(1),
            _ => unreachable!(),
        }
        assert!(
            verify_module(
                &forged,
                &ProofBundle::default(),
                &AdmissionProfile::default()
            )
            .is_err(),
            "view corruption {corruption}"
        );
    }
}

fn scalar_case_call_module() -> TerminalModule {
    let mut module = payloadless_call_module();
    let scalar_type = ScalarType::Integer(count_type());
    let StructuralTypeShape::Sum { cases } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    cases[0].fields = (1..=2)
        .map(|identity| StructuralFieldDeclaration {
            id: StructuralFieldId::new(identity).unwrap(),
            identity: format!("field_{identity}"),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Scalar(scalar_type),
        })
        .collect();
    let callee = &mut module.machines[1];
    callee.parameters = vec![
        ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(20),
            scalar_type,
        },
        ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(21),
            scalar_type,
        },
    ];
    let TerminalMachineResult::Structural(result) = &mut callee.result else {
        unreachable!()
    };
    result.multiplicity = StructuralMultiplicity::Affine;
    let OperationResult::Structural(result) = &mut callee.blocks[0].operations[0].result else {
        unreachable!()
    };
    result.multiplicity = StructuralMultiplicity::Affine;
    callee.blocks[0].operations[0].kind = OperationKind::EstablishScalarCase {
        result_case: structural_case_id(1),
        fields: vec![
            terminal_psi::ScalarCaseField {
                field: StructuralFieldId::new(1).unwrap(),
                value: value_id(21),
                range_obligation: None,
            },
            terminal_psi::ScalarCaseField {
                field: StructuralFieldId::new(2).unwrap(),
                value: value_id(20),
                range_obligation: None,
            },
        ],
    };
    let caller = &mut module.machines[0];
    caller.parameters = vec![
        ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(10),
            scalar_type,
        },
        ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(11),
            scalar_type,
        },
    ];
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(40),
        scalar_type,
    });
    caller
        .structural_places
        .retain(|place| place.kind != semantic_vocabulary::StructuralPlaceKind::Result);
    let OperationResult::Structural(result) = &mut caller.blocks[0].operations[0].result else {
        unreachable!()
    };
    result.multiplicity = StructuralMultiplicity::Affine;
    caller.blocks[0].operations[0].kind = OperationKind::CallStructuralWithScalarArguments {
        callee: machine_id(2),
        arguments: vec![value_id(10), value_id(11)],
        structural_arguments: vec![],
        claim_transfers: vec![],
        returned_claim_transfers: vec![],
        requirement_obligations: vec![],
        crash_continuations: vec![],
    };
    caller.blocks[0].terminator = Terminator::StructuralCase {
        source: place_id(3),
        cases: vec![terminal_psi::StructuralCaseSuccessorEdge {
            edge: edge_id(1),
            target: block_id(3),
            case: structural_case_id(1),
            payload_fields: vec![
                StructuralFieldId::new(2).unwrap(),
                StructuralFieldId::new(1).unwrap(),
            ],
            trivial_affine_discards: vec![place_id(3)],
        }],
    };
    caller.blocks.push(Block {
        id: block_id(3),
        structural_parameters: vec![],
        parameters: vec![
            ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(30),
                scalar_type,
            },
            ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(31),
                scalar_type,
            },
        ],
        operations: vec![],
        terminator: Terminator::Return {
            edge: edge_id(3),
            value: value_id(30),
            cleanup_actions: vec![],
        },
    });
    module
}

#[test]
fn scalar_case_constructor_returns_runtime_fields_through_call_and_reordered_inspection() {
    let module = scalar_case_call_module();
    let semantic = encode_module(&module).unwrap();
    let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[count(17), count(83)],
    )
    .unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(2);
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(10).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(count(17)))
    );
    assert_eq!(
        meter
            .usage()
            .at(FuelChargeSite::Operation(operation_id(1)))
            .unwrap()
            .units(),
        1
    );
}

#[test]
fn scalar_case_constructor_rejects_incomplete_duplicate_wrong_type_and_undefined_payloads() {
    let module = scalar_case_call_module();
    for corruption in 0..5 {
        let mut forged = module.clone();
        let OperationKind::EstablishScalarCase { fields, .. } =
            &mut forged.machines[1].blocks[0].operations[0].kind
        else {
            unreachable!()
        };
        match corruption {
            0 => {
                fields.pop();
            }
            1 => fields[1].field = fields[0].field,
            2 => fields[0].value = value_id(99),
            3 => fields[0].range_obligation = Some(obligation_id(1)),
            4 => forged.machines[1].parameters[1].scalar_type = ScalarType::Boolean,
            _ => unreachable!(),
        }
        assert!(
            verify_module(
                &forged,
                &ProofBundle::default(),
                &AdmissionProfile::default()
            )
            .is_err(),
            "corruption {corruption}"
        );
    }
}

#[test]
fn scalar_case_constructor_rejects_linear_creation_and_missing_affine_disposal() {
    let mut forged = scalar_case_call_module();
    let OperationResult::Structural(result) =
        &mut forged.machines[1].blocks[0].operations[0].result
    else {
        unreachable!()
    };
    result.multiplicity = StructuralMultiplicity::Linear;
    assert!(
        verify_module(
            &forged,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err()
    );
    let mut forged = scalar_case_call_module();
    let Terminator::StructuralCase { cases, .. } = &mut forged.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cases[0].trivial_affine_discards.clear();
    assert!(
        verify_module(
            &forged,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err()
    );
}

fn bounded_constructor() -> (TerminalModule, ProofBundle) {
    let mut module = scalar_case_call_module();
    let mut callee = module.machines.remove(1);
    module.entry = callee.id;
    let StructuralTypeShape::Sum { cases } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    cases[0].fields[0].field_type = StructuralFieldType::BoundedInteger(
        semantic_vocabulary::BoundedIntegerType::new(
            count_type(),
            IntegerValue::Unsigned(0),
            IntegerValue::Unsigned(100),
        )
        .unwrap(),
    );
    let OperationKind::EstablishScalarCase { fields, .. } =
        &mut callee.blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    fields[0].range_obligation = Some(obligation_id(1));
    let term = ScalarTerm::value(value_id(21), ScalarType::Integer(count_type()));
    let mut bounds = vec![
        Proposition::LessOrEqual(
            ScalarTerm::integer(count_type(), IntegerValue::Unsigned(0)).unwrap(),
            term.clone(),
        ),
        Proposition::LessOrEqual(
            term,
            ScalarTerm::integer(count_type(), IntegerValue::Unsigned(100)).unwrap(),
        ),
    ];
    bounds.sort();
    let conclusion = Proposition::Conjunction(bounds);
    callee.contract.requires = vec![conclusion.clone()];
    module.machines = vec![callee];
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion,
                    rule: ProofRule::Assumption { index: 0 },
                },
            }),
        }],
        ..ProofBundle::default()
    };
    (module, bundle)
}

#[test]
fn scalar_case_constructor_checks_exact_range_before_any_result_facts() {
    let (module, bundle) = bounded_constructor();
    let reconstructed = terminal_verifier::reconstruct_terminal_obligations(&module).unwrap();
    let site = reconstructed
        .obligations()
        .iter()
        .find(|site| site.obligation.id == obligation_id(1))
        .unwrap();
    assert_eq!(
        site.obligation.proposition,
        module.machines[0].contract.requires[0]
    );
    assert!(
        site.semantic_axioms.is_empty(),
        "the new result cannot prove its own initializer"
    );
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    let measured = interpret_terminal_artifact_measured(
        &encode_module(&module).unwrap(),
        &encode_proof_bundle(&bundle).unwrap(),
        &AdmissionProfile::default(),
        &[count(17), count(83)],
    )
    .unwrap();
    let TerminalExecutionResult::ScalarCase(returned) = measured.value() else {
        panic!("scalar case")
    };
    assert_eq!(
        returned.value.fields,
        vec![
            (StructuralFieldId::new(1).unwrap(), count(83)),
            (StructuralFieldId::new(2).unwrap(), count(17))
        ]
    );
    assert!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err()
    );
    let mut forged = module.clone();
    let OperationKind::EstablishScalarCase { fields, .. } =
        &mut forged.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    fields[0].value = value_id(20);
    assert!(
        verify_module(&forged, &bundle, &AdmissionProfile::default()).is_err(),
        "same carrier does not authorize another value"
    );
    let mut forged = module.clone();
    let StructuralTypeShape::Sum { cases } = &mut forged.structural_types[0].shape else {
        unreachable!()
    };
    cases[0].fields[0].field_type = StructuralFieldType::BoundedInteger(
        semantic_vocabulary::BoundedIntegerType::new(
            count_type(),
            IntegerValue::Unsigned(0),
            IntegerValue::Unsigned(50),
        )
        .unwrap(),
    );
    assert!(
        verify_module(&forged, &bundle, &AdmissionProfile::default()).is_err(),
        "narrowing changes the exact proof goal"
    );
}

#[test]
fn scalar_case_constructor_result_disposes_on_ordinary_returns_and_edges() {
    for route in 0..4 {
        let mut module = scalar_case_call_module();
        let caller = &mut module.machines[0];
        match route {
            0 => {
                caller.blocks.truncate(1);
                caller.result = TerminalMachineResult::Unit;
                caller.blocks[0].terminator = Terminator::ReturnUnit {
                    edge: edge_id(1),
                    trivial_affine_discards: vec![place_id(3)],
                };
            }
            1 => {
                caller.blocks.truncate(1);
                caller.blocks[0].terminator = Terminator::Return {
                    edge: edge_id(1),
                    value: value_id(10),
                    cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(place_id(3))],
                };
            }
            2 => {
                caller.blocks[0].terminator = Terminator::Jump {
                    edge: edge_id(1),
                    target: block_id(3),
                    arguments: vec![value_id(10), value_id(11)],
                    structural_arguments: vec![],
                    trivial_affine_discards: vec![place_id(3)],
                    residual_affine_discards: vec![],
                };
            }
            3 => {
                caller.blocks[0].operations.push(Operation {
                    static_reach_binding: None,
                    id: operation_id(3),
                    result: OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: value_id(12),
                        scalar_type: ScalarType::Boolean,
                    }),
                    kind: OperationKind::BooleanConstant { value: true },
                });
                let successor = |edge| SuccessorEdge {
                    edge: edge_id(edge),
                    target: block_id(3),
                    arguments: vec![value_id(10), value_id(11)],
                    structural_arguments: vec![],
                    trivial_affine_discards: vec![place_id(3)],
                };
                caller.blocks[0].terminator = Terminator::Conditional {
                    condition: value_id(12),
                    when_true: successor(1),
                    when_false: successor(4),
                };
            }
            _ => unreachable!(),
        }
        let measured = interpret_terminal_artifact_measured(
            &encode_module(&module).unwrap(),
            &encode_proof_bundle(&ProofBundle::default()).unwrap(),
            &AdmissionProfile::default(),
            &[count(17), count(83)],
        )
        .unwrap();
        assert_eq!(
            measured.value(),
            if route == 0 {
                TerminalExecutionResult::Unit
            } else {
                TerminalExecutionResult::Scalar(count(17))
            }
        );
    }
}

#[test]
fn scalar_case_constructor_returns_from_a_checked_unranked_loop() {
    let mut module = scalar_case_call_module();
    let mut machine = module.machines.remove(1);
    module.entry = machine.id;
    let scalar_type = ScalarType::Integer(count_type());
    let mut returning = machine.blocks.remove(0);
    returning.id = block_id(6);
    let OperationKind::EstablishScalarCase { fields, .. } = &mut returning.operations[0].kind
    else {
        unreachable!()
    };
    fields[0].value = value_id(23);
    fields[1].value = value_id(22);
    let jump = |edge, target, arguments| Terminator::Jump {
        edge: edge_id(edge),
        target: block_id(target),
        arguments,
        structural_arguments: vec![],
        trivial_affine_discards: vec![],
        residual_affine_discards: vec![],
    };
    let constant = |operation, value, integer| Operation {
        static_reach_binding: None,
        id: operation_id(operation),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(value),
            scalar_type,
        }),
        kind: OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(integer),
        },
    };
    let successor = |edge, target| SuccessorEdge {
        edge: edge_id(edge),
        target: block_id(target),
        arguments: vec![],
        structural_arguments: vec![],
        trivial_affine_discards: vec![],
    };
    machine.blocks = vec![
        Block {
            id: block_id(2),
            parameters: vec![],
            structural_parameters: vec![],
            operations: vec![],
            terminator: jump(3, 4, vec![value_id(20), value_id(21)]),
        },
        Block {
            id: block_id(4),
            structural_parameters: vec![],
            parameters: vec![
                ValueDeclaration {
                    qualifications: Default::default(),
                    id: value_id(22),
                    scalar_type,
                },
                ValueDeclaration {
                    qualifications: Default::default(),
                    id: value_id(23),
                    scalar_type,
                },
            ],
            operations: vec![
                constant(3, 24, 0),
                Operation {
                    static_reach_binding: None,
                    id: operation_id(4),
                    result: OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: value_id(25),
                        scalar_type: ScalarType::Boolean,
                    }),
                    kind: OperationKind::IntegerLessThan {
                        left: value_id(24),
                        right: value_id(22),
                    },
                },
            ],
            terminator: Terminator::Conditional {
                condition: value_id(25),
                when_true: successor(4, 5),
                when_false: successor(5, 6),
            },
        },
        Block {
            id: block_id(5),
            structural_parameters: vec![],
            parameters: vec![],
            operations: vec![
                constant(5, 26, 1),
                Operation {
                    static_reach_binding: None,
                    id: operation_id(6),
                    result: OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: value_id(27),
                        scalar_type,
                    }),
                    kind: OperationKind::WrappingIntegerSubtract {
                        left: value_id(22),
                        right: value_id(26),
                    },
                },
            ],
            terminator: jump(6, 4, vec![value_id(27), value_id(23)]),
        },
        returning,
    ];
    module.machines = vec![machine];
    let measured = interpret_terminal_artifact_measured(
        &encode_module(&module).unwrap(),
        &encode_proof_bundle(&ProofBundle::default()).unwrap(),
        &AdmissionProfile::default(),
        &[count(3), count(83)],
    )
    .unwrap();
    let TerminalExecutionResult::ScalarCase(returned) = measured.value() else {
        panic!("scalar case")
    };
    assert_eq!(
        returned.value.fields,
        vec![
            (StructuralFieldId::new(1).unwrap(), count(83)),
            (StructuralFieldId::new(2).unwrap(), count(0))
        ]
    );
}

#[path = "scalar_cases/owned_state.rs"]
mod owned_state;
