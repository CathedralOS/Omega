use super::byte_sequence_read::{
    emit_byte, finish, guarded_module, integer, scalar, successor, unsigned_type,
};
use super::*;

fn result(place: u64) -> OperationResult {
    OperationResult::Structural(terminal_psi::StructuralOperationResult {
        place: place_id(place),
        structural_type: structural_type_id(1),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    })
}

fn declaration(place: u64, producer: u64) -> StructuralPlaceDeclaration {
    StructuralPlaceDeclaration {
        id: place_id(place),
        kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
            producer: operation_id(producer),
            structural_type: structural_type_id(1),
        },
    }
}

fn jump(edge: u64, target: u64) -> Terminator {
    Terminator::Jump {
        structural_arguments: Vec::new(),
        edge: edge_id(edge),
        target: block_id(target),
        arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    }
}

fn length_effect(ordinal: u64, value: u64) -> Operation {
    let mut operation = emit_byte(ordinal, value);
    if let OperationKind::BoundaryCall { boundary, .. } = &mut operation.kind {
        *boundary = boundary_id(2);
    }
    operation
}

fn module(bytes: Vec<u8>) -> TerminalModule {
    let mut module = guarded_module(bytes, 1);
    let mut length_boundary = module.boundary_machines[0].clone();
    length_boundary.id = boundary_id(2);
    length_boundary.identity = "test::length".into();
    length_boundary.scalar_parameters = vec![unsigned_type(64)];
    module.boundary_machines.push(length_boundary);
    let mut inspector = module.machines[1].clone();
    let helper = &mut module.machines[1];
    helper.structural_places.push(declaration(4, 12));
    helper.blocks[0].operations[1].kind = OperationKind::IntegerLessOrEqual {
        left: value_id(20),
        right: value_id(10),
    };
    helper.blocks[1].operations = vec![
        Operation {
            id: operation_id(12),
            result: result(4),
            kind: OperationKind::ByteSequenceSubslice {
                source: place_id(3),
                start: value_id(20),
                end: value_id(10),
                length: value_id(10),
                obligation: obligation_id(1),
            },
        },
        integer(14, 64, 0),
        Operation {
            id: operation_id(13),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                callee: machine_id(3),
                arguments: vec![value_id(14)],
                structural_arguments: vec![StructuralArgument {
                    place: place_id(4),
                    path: Vec::new(),
                    access: StructuralAccess::SharedBorrow,
                }],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        },
    ];
    helper.blocks[1].terminator = jump(4, 5);
    helper.blocks[2].terminator = jump(5, 5);
    helper.blocks.push(Block {
        structural_parameters: Vec::new(),
        id: block_id(5),
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator: finish(6),
    });
    inspector.id = machine_id(3);
    inspector.contract.id = contract_id(3);
    inspector.entry = block_id(6);
    inspector.parameters = vec![scalar(50, 64)];
    inspector.structural_places[0].id = place_id(5);
    inspector.structural_parameters[0].place = place_id(5);
    inspector.structural_places.push(declaration(6, 35));
    inspector.blocks = vec![
        Block {
            structural_parameters: Vec::new(),
            id: block_id(6),
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    id: operation_id(30),
                    result: OperationResult::Scalar(scalar(30, 64)),
                    kind: OperationKind::ByteSequenceLength {
                        source: place_id(5),
                    },
                },
                length_effect(34, 30),
                Operation {
                    id: operation_id(31),
                    result: OperationResult::Scalar(ValueDeclaration {
                        id: value_id(31),
                        scalar_type: ScalarType::Boolean,
                    }),
                    kind: OperationKind::IntegerLessThan {
                        left: value_id(50),
                        right: value_id(30),
                    },
                },
            ],
            terminator: Terminator::Conditional {
                condition: value_id(31),
                when_true: successor(7, 7),
                when_false: successor(8, 8),
            },
        },
        Block {
            structural_parameters: Vec::new(),
            id: block_id(7),
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    id: operation_id(35),
                    result: result(6),
                    kind: OperationKind::ByteSequenceSubslice {
                        source: place_id(5),
                        start: value_id(50),
                        end: value_id(30),
                        length: value_id(30),
                        obligation: obligation_id(3),
                    },
                },
                Operation {
                    id: operation_id(36),
                    result: OperationResult::Scalar(scalar(36, 64)),
                    kind: OperationKind::ByteSequenceLength {
                        source: place_id(6),
                    },
                },
                Operation {
                    id: operation_id(37),
                    result: OperationResult::Scalar(ValueDeclaration {
                        id: value_id(37),
                        scalar_type: ScalarType::Boolean,
                    }),
                    kind: OperationKind::IntegerLessThan {
                        left: value_id(50),
                        right: value_id(36),
                    },
                },
            ],
            terminator: Terminator::Conditional {
                condition: value_id(37),
                when_true: successor(9, 9),
                when_false: successor(11, 8),
            },
        },
        Block {
            structural_parameters: Vec::new(),
            id: block_id(8),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: finish(10),
        },
        Block {
            structural_parameters: Vec::new(),
            id: block_id(9),
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    id: operation_id(32),
                    result: OperationResult::Scalar(scalar(32, 8)),
                    kind: OperationKind::ByteSequenceRead {
                        source: place_id(6),
                        index: value_id(50),
                        length: value_id(36),
                        obligation: obligation_id(2),
                    },
                },
                emit_byte(33, 32),
            ],
            terminator: finish(12),
        },
    ];
    module.machines.push(inspector);
    module
}

fn prove(goal: &Proposition, axioms: &[Proposition]) -> ProofNode {
    let rule = if let Some(index) = axioms.iter().position(|fact| fact == goal) {
        ProofRule::SemanticAxiom { index }
    } else {
        match goal {
            Proposition::Conjunction(children) => ProofRule::ConjunctionIntroduction(
                children.iter().map(|child| prove(child, axioms)).collect(),
            ),
            Proposition::LessOrEqual(left, right) if left == right => {
                ProofRule::IntegerOrderWeakening {
                    relation: Box::new(ProofNode {
                        conclusion: Proposition::Equal(left.clone(), right.clone()),
                        rule: ProofRule::Primitive(
                            proof_admission::PrimitiveJudgment::ReflexiveEquality,
                        ),
                    }),
                }
            }
            Proposition::LessOrEqual(left, right) => ProofRule::IntegerOrderWeakening {
                relation: Box::new(prove(
                    &Proposition::LessThan(left.clone(), right.clone()),
                    axioms,
                )),
            },
            _ => panic!("fixture requires exact selected fact: {goal:?}"),
        }
    };
    ProofNode {
        conclusion: goal.clone(),
        rule,
    }
}

pub(super) fn certificate(module: &TerminalModule) -> ProofBundle {
    let questions = terminal_verifier::reconstruct_terminal_obligations(module).unwrap();
    let mut evidence = questions
        .obligations()
        .iter()
        .map(|site| {
            assert!(site.canonical_certificate);
            ObligationEvidence {
                obligation: site.obligation.id,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(site.obligation.id.get()).unwrap(),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: prove(&site.obligation.proposition, &site.semantic_axioms),
                }),
            }
        })
        .collect::<Vec<_>>();
    evidence.sort_by_key(|row| row.obligation);
    let proof = ProofBundle {
        evidence,
        ..ProofBundle::default()
    };
    verify_module(module, &proof, &AdmissionProfile::default()).unwrap();
    proof
}

#[test]
fn guarded_subslice_calls_keep_bytes_empty_tails_nested_views_joins_and_fuel() {
    for (bytes, start, expected, ran) in [
        (vec![], 1, vec![7], false),
        (vec![255], 1, vec![0, 7], true),
        (vec![0, 128, 255], 1, vec![2, 128, 7], true),
        (vec![128, 0], 1, vec![1, 0, 7], true),
        (vec![], 0, vec![0, 7], true),
        (vec![255, 0], 0, vec![2, 255, 7], true),
    ] {
        let mut module = module(bytes);
        module.machines[0].blocks[0].operations[1] = integer(2, 64, start);
        let semantic = encode_module(&module).unwrap();
        assert_eq!(decode_module(&semantic).unwrap(), module);
        let proof = encode_proof_bundle(&certificate(&module)).unwrap();
        let mut prior = None;
        for incremental in [false, true] {
            let mut execution = TerminalExecution::start_artifact(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                &[],
            )
            .unwrap();
            let mut meter = if incremental {
                TerminalFuelMeter::with_allowance(0)
            } else {
                TerminalFuelMeter::unbounded()
            };
            let mut handler = RecordingHandler::default();
            loop {
                match execution
                    .resume_with_effect_handler(&mut meter, &mut handler)
                    .unwrap()
                {
                    TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
                    TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => break,
                    status => panic!("unexpected {status:?}"),
                }
            }
            let actual = handler
                .effects
                .iter()
                .map(|effect| {
                    let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
                        panic!("boundary")
                    };
                    let TerminalScalarValue::Integer {
                        value: IntegerValue::Unsigned(value),
                        ..
                    } = arguments[0]
                    else {
                        panic!("integer")
                    };
                    value
                })
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
            assert_eq!(
                meter
                    .usage()
                    .at(FuelChargeSite::Operation(operation_id(12)))
                    .map(|charge| charge.units()),
                ran.then_some(1)
            );
            if let Some(usage) = &prior {
                assert_eq!(meter.usage(), usage);
            } else {
                prior = Some(meter.usage().clone());
            }
        }
    }
}

fn slice(module: &mut TerminalModule) -> &mut Operation {
    &mut module.machines[1].blocks[1].operations[0]
}

#[test]
fn subslice_boundary_receives_only_the_window_and_preserves_caller_continuation() {
    for (bytes, expected) in [(vec![0, 128, 255], vec![128, 255]), (vec![255], vec![])] {
        let mut module = module(bytes);
        let mut boundary = module.boundary_machines[0].clone();
        boundary.id = boundary_id(3);
        boundary.identity = "test::window".into();
        boundary.scalar_parameters.clear();
        let mut parameter = module.machines[1].structural_parameters[0].clone();
        parameter.place = place_id(8);
        boundary.structural_parameters = vec![parameter];
        module.boundary_machines.push(boundary);
        module.machines[1].blocks[1].operations.insert(
            1,
            Operation {
                id: operation_id(15),
                result: OperationResult::Unit,
                kind: OperationKind::BoundaryCall {
                    boundary: boundary_id(3),
                    arguments: Vec::new(),
                    structural_arguments: vec![StructuralArgument {
                        place: place_id(4),
                        path: Vec::new(),
                        access: StructuralAccess::SharedBorrow,
                    }],
                    completion_receipts: Vec::new(),
                },
            },
        );
        let proof = encode_proof_bundle(&certificate(&module)).unwrap();
        let execution = interpret_terminal_artifact_measured(
            &encode_module(&module).unwrap(),
            &proof,
            &AdmissionProfile::default(),
            &[],
        )
        .unwrap();
        let TerminalEffect::BoundaryCall {
            byte_sequence_arguments,
            ..
        } = &execution.effects()[0]
        else {
            panic!("boundary")
        };
        assert_eq!(byte_sequence_arguments, &[Some(expected)]);
        let TerminalEffect::BoundaryCall { arguments, .. } = execution.effects().last().unwrap()
        else {
            panic!("continuation")
        };
        assert!(matches!(
            arguments.as_slice(),
            [TerminalScalarValue::Integer {
                value: IntegerValue::Unsigned(7),
                ..
            }]
        ));
    }
}

#[test]
fn subslice_rejects_fake_wrong_source_later_sibling_and_reobserved_lengths() {
    let base = module(vec![0, 128, 255]);
    let proof = certificate(&base);
    for placement in 0..5 {
        let mut changed = base.clone();
        let mut length = Operation {
            id: operation_id(15),
            result: OperationResult::Scalar(scalar(15, 64)),
            kind: OperationKind::ByteSequenceLength {
                source: place_id(3),
            },
        };
        if placement == 0 {
            length = integer(15, 64, 999);
        }
        if let OperationKind::ByteSequenceSubslice { end, length, .. } =
            &mut slice(&mut changed).kind
        {
            *end = value_id(15);
            *length = value_id(15);
        }
        match placement {
            0 | 4 => changed.machines[1].blocks[0].operations.insert(0, length),
            1 => changed.machines[1].blocks[1].operations.push(length),
            2 => changed.machines[1].blocks[2].operations.push(length),
            3 => {
                let helper = &mut changed.machines[1];
                let mut parameter = helper.structural_parameters[0].clone();
                parameter.place = place_id(7);
                parameter.position = 1;
                helper.structural_parameters.push(parameter);
                helper.structural_places.push(StructuralPlaceDeclaration {
                    id: place_id(7),
                    kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                        position: 1,
                        is_self: false,
                    },
                });
                length.kind = OperationKind::ByteSequenceLength {
                    source: place_id(7),
                };
                helper.blocks[0].operations.insert(0, length);
                if let OperationKind::CallUnit {
                    structural_arguments,
                    ..
                } = &mut changed.machines[0].blocks[0].operations[2].kind
                {
                    structural_arguments.push(structural_arguments[0].clone());
                }
            }
            _ => unreachable!(),
        }
        if placement == 4 {
            terminal_verifier::validate_module_representation(&changed).unwrap();
            assert!(verify_module(&changed, &proof, &AdmissionProfile::default()).is_err());
        } else {
            assert!(terminal_verifier::validate_module_representation(&changed).is_err());
        }
    }
    for operand in [20, 999] {
        let mut changed = base.clone();
        if let OperationKind::ByteSequenceSubslice { length, .. } = &mut slice(&mut changed).kind {
            *length = value_id(operand);
        }
        assert!(terminal_verifier::validate_module_representation(&changed).is_err());
    }
}

#[test]
fn subslice_requires_exact_borrowed_result_and_never_grants_other_argument_access() {
    let base = module(vec![0, 128]);
    for mutation in 0..11 {
        let mut changed = base.clone();
        match mutation {
            0 => slice(&mut changed).result = OperationResult::Unit,
            1 => slice(&mut changed).result = OperationResult::Scalar(scalar(12, 8)),
            2 => {
                if let OperationResult::Structural(result) = &mut slice(&mut changed).result {
                    result.multiplicity = StructuralMultiplicity::Affine;
                }
            }
            3 => {
                if let OperationResult::Structural(result) = &mut slice(&mut changed).result {
                    result
                        .qualifications
                        .push(StructuralDomainId::new(1).unwrap());
                }
            }
            4 => changed.machines[1].structural_places[1] = declaration(4, 999),
            5 => {
                changed.machines[1].structural_parameters[0].access =
                    StructuralAccess::MutableBorrow
            }
            6 | 7 => {
                if let OperationKind::CallUnit {
                    structural_arguments,
                    ..
                } = &mut changed.machines[1].blocks[1].operations[2].kind
                {
                    structural_arguments[0].access = if mutation == 6 {
                        StructuralAccess::Owned
                    } else {
                        StructuralAccess::MutableBorrow
                    };
                }
            }
            8 => {
                if let OperationKind::CallUnit {
                    structural_arguments,
                    ..
                } = &mut changed.machines[1].blocks[1].operations[2].kind
                {
                    structural_arguments[0]
                        .path
                        .push(StructuralPathSegment::FixedIndex(0));
                }
            }
            9 => {
                if let OperationResult::Structural(result) = &mut slice(&mut changed).result {
                    result.claims.push(StructuralResultClaimBinding {
                        claim: claim_id(1),
                        path: Vec::new(),
                    });
                }
            }
            10 => changed.machines[1].entry_claims.push(EntryClaim {
                claim: claim_id(1),
                input: place_id(3),
                path: Vec::new(),
            }),
            _ => unreachable!(),
        }
        assert!(
            terminal_verifier::validate_module_representation(&changed).is_err(),
            "mutation {mutation}"
        );
        assert!(encode_module(&changed).is_err(), "mutation {mutation}");
    }
}

#[test]
fn subslice_result_must_dominate_calls_reads_lengths_and_nested_slices() {
    let base = module(vec![0, 128]);
    for mutation in 0..5 {
        let mut changed = base.clone();
        match mutation {
            0 => {
                let operation = changed.machines[1].blocks[1].operations.remove(0);
                changed.machines[1].blocks[2].operations.push(operation);
            }
            1 => {
                let operation = changed.machines[1].blocks[1].operations.remove(0);
                changed.machines[1].blocks[1].operations.push(operation);
            }
            2 => {
                let call = changed.machines[1].blocks[1].operations.pop().unwrap();
                changed.machines[1].blocks[3].operations.push(call);
            }
            3 => {
                let operation = changed.machines[2].blocks[1].operations.remove(0);
                changed.machines[2].blocks[2].operations.push(operation);
            }
            4 => {
                changed.machines[1].blocks[1].operations.remove(0);
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_verifier::validate_module_representation(&changed).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn subslice_bounds_need_canonical_selected_certificates_and_unique_obligations() {
    let base = module(vec![0, 128]);
    let proof = certificate(&base);
    assert!(verify_module(&base, &ProofBundle::default(), &AdmissionProfile::default()).is_err());
    let mut retargeted = proof.clone();
    retargeted.evidence[0].obligation = obligation_id(99);
    assert!(verify_module(&base, &retargeted, &AdmissionProfile::default()).is_err());
    for mutation in 0..4 {
        let mut changed = base.clone();
        match mutation {
            0 => {
                if let OperationKind::ByteSequenceSubslice { start, end, .. } =
                    &mut slice(&mut changed).kind
                {
                    std::mem::swap(start, end);
                }
            }
            1 => {
                changed.machines[1].blocks[0].terminator = Terminator::Conditional {
                    condition: value_id(11),
                    when_true: successor(2, 4),
                    when_false: successor(3, 3),
                };
            }
            2 => {
                let operations = std::mem::take(&mut changed.machines[1].blocks[1].operations);
                changed.machines[1].blocks[3].operations = operations;
            }
            3 => {
                if let OperationKind::ByteSequenceSubslice { end, .. } =
                    &mut slice(&mut changed).kind
                {
                    *end = value_id(15);
                }
                changed.machines[1].blocks[0]
                    .operations
                    .insert(0, integer(15, 64, u128::from(u64::MAX)));
            }
            _ => unreachable!(),
        }
        terminal_verifier::validate_module_representation(&changed).unwrap();
        assert!(
            verify_module(&changed, &proof, &AdmissionProfile::default()).is_err(),
            "mutation {mutation}"
        );
    }
    let mut duplicate = base;
    if let OperationKind::ByteSequenceSubslice { obligation, .. } =
        &mut duplicate.machines[2].blocks[1].operations[0].kind
    {
        *obligation = obligation_id(1);
    }
    assert!(matches!(
        terminal_verifier::validate_module_representation(&duplicate),
        Err(ModuleError::DuplicateObligation(_))
    ));
}

#[test]
fn subslice_types_and_borrowed_return_are_not_implicitly_supported() {
    let base = module(vec![0, 128]);
    for bits in [8, 32, 128] {
        for position in 0..3 {
            let mut changed = base.clone();
            changed.machines[1].blocks[0]
                .operations
                .insert(0, integer(15, bits, 1));
            if let OperationKind::ByteSequenceSubslice {
                start, end, length, ..
            } = &mut slice(&mut changed).kind
            {
                *match position {
                    0 => start,
                    1 => end,
                    _ => length,
                } = value_id(15);
            }
            assert!(terminal_verifier::validate_module_representation(&changed).is_err());
        }
    }
    let mut signed = base.clone();
    signed.machines[1].blocks[0].operations.insert(
        0,
        Operation {
            id: operation_id(15),
            result: OperationResult::Scalar(ValueDeclaration {
                id: value_id(15),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Signed, 64).unwrap(),
                ),
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Signed(-1),
            },
        },
    );
    if let OperationKind::ByteSequenceSubslice { start, .. } = &mut slice(&mut signed).kind {
        *start = value_id(15);
    }
    assert!(terminal_verifier::validate_module_representation(&signed).is_err());
    let mut returned = base;
    returned.machines[1].blocks[1].terminator = Terminator::ReturnStructural {
        edge: edge_id(4),
        source: place_id(4),
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    assert!(matches!(
        terminal_verifier::validate_module_representation(&returned),
        Err(ModuleError::ByteSequenceSubsliceReturnUnsupported { .. })
    ));
}

#[test]
fn subslice_wire_and_certificate_tampering_reject_before_execution() {
    let module = module(vec![0, 128]);
    let proof = encode_proof_bundle(&certificate(&module)).unwrap();
    let bytes = encode_module(&module).unwrap();
    assert_eq!(&bytes[10..12], &86_u16.to_le_bytes());
    let pattern = [
        vec![57],
        3_u64.to_le_bytes().to_vec(),
        20_u64.to_le_bytes().to_vec(),
        10_u64.to_le_bytes().to_vec(),
        10_u64.to_le_bytes().to_vec(),
        1_u64.to_le_bytes().to_vec(),
    ]
    .concat();
    let positions = bytes
        .windows(pattern.len())
        .enumerate()
        .filter_map(|(position, row)| (row == pattern).then_some(position))
        .collect::<Vec<_>>();
    let [position] = positions.as_slice() else {
        panic!("one exact subslice")
    };
    for operand in 0..5 {
        let mut corrupt = bytes.clone();
        let offset = position + 1 + operand * 8;
        corrupt[offset..offset + 8].copy_from_slice(&999_u64.to_le_bytes());
        assert!(
            TerminalExecution::start_artifact(&corrupt, &proof, &AdmissionProfile::default(), &[])
                .is_err()
        );
    }
    let mut stale = bytes.clone();
    stale[10..12].copy_from_slice(&82_u16.to_le_bytes());
    assert!(
        TerminalExecution::start_artifact(&stale, &proof, &AdmissionProfile::default(), &[])
            .is_err()
    );
    let mut forged = certificate(&module);
    let EvidenceRoute::CertificateDerived(certificate) = &mut forged.evidence[0].route else {
        panic!("certificate")
    };
    let Proposition::Conjunction(children) = &mut certificate.proof.conclusion else {
        panic!("conjunction")
    };
    children.reverse();
    assert!(
        TerminalExecution::start_artifact(
            &bytes,
            &encode_proof_bundle(&forged).unwrap(),
            &AdmissionProfile::default(),
            &[]
        )
        .is_err()
    );
}
