use super::*;

pub(super) fn unsigned_type(bits: u16) -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap())
}

pub(super) fn scalar(ordinal: u64, bits: u16) -> ValueDeclaration {
    ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(ordinal),
        scalar_type: unsigned_type(bits),
    }
}

pub(super) fn integer(ordinal: u64, bits: u16, value: u128) -> Operation {
    Operation {
        id: operation_id(ordinal),
        result: OperationResult::Scalar(scalar(ordinal, bits)),
        kind: OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(value),
        },
    }
}

pub(super) fn emit_byte(ordinal: u64, value: u64) -> Operation {
    Operation {
        id: operation_id(ordinal),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            boundary: boundary_id(1),
            arguments: vec![value_id(value)],
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
        },
    }
}

pub(super) fn finish(edge: u64) -> Terminator {
    Terminator::ReturnUnit {
        edge: edge_id(edge),
        trivial_affine_discards: Vec::new(),
    }
}

pub(super) fn successor(edge: u64, block: u64) -> SuccessorEdge {
    SuccessorEdge {
        structural_arguments: Vec::new(),
        edge: edge_id(edge),
        target: block_id(block),
        arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}

/// Literal establishment stays in the caller; the helper owns its invocation's
/// immutable view and a genuine conditional read. The final byte is emitted
/// only after normal caller continuation.
pub(super) fn guarded_module(bytes: Vec<u8>, byte_index: u64) -> TerminalModule {
    let mut module = byte_sequence_literal_module(bytes);
    module.boundary_machines[0].structural_parameters.clear();
    module.boundary_machines[0].scalar_parameters = vec![unsigned_type(8)];
    let mut helper = module.machines[0].clone();
    let caller = &mut module.machines[0];
    caller.blocks[0].operations.truncate(1);
    caller.blocks[0].operations.extend([
        integer(2, 64, u128::from(byte_index)),
        Operation {
            id: operation_id(3),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                callee: machine_id(2),
                arguments: vec![value_id(2)],
                structural_arguments: vec![StructuralArgument {
                    place: place_id(1),
                    path: Vec::new(),
                    access: StructuralAccess::SharedBorrow,
                }],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        },
        integer(4, 8, 7),
        emit_byte(5, 4),
    ]);
    helper.id = machine_id(2);
    helper.entry = block_id(2);
    helper.contract.id = contract_id(2);
    helper.parameters = vec![scalar(20, 64)];
    helper.structural_places = vec![StructuralPlaceDeclaration {
        id: place_id(3),
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    helper.structural_parameters = vec![StructuralParameterDeclaration {
        place: place_id(3),
        position: 0,
        is_self: false,
        structural_type: structural_type_id(1),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    helper.blocks = vec![
        Block {
            structural_parameters: Vec::new(),
            id: block_id(2),
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    id: operation_id(10),
                    result: OperationResult::Scalar(scalar(10, 64)),
                    kind: OperationKind::ByteSequenceLength {
                        source: place_id(3),
                    },
                },
                Operation {
                    id: operation_id(11),
                    result: OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: value_id(11),
                        scalar_type: ScalarType::Boolean,
                    }),
                    kind: OperationKind::IntegerLessThan {
                        left: value_id(20),
                        right: value_id(10),
                    },
                },
            ],
            terminator: Terminator::Conditional {
                condition: value_id(11),
                when_true: successor(2, 3),
                when_false: successor(3, 4),
            },
        },
        Block {
            structural_parameters: Vec::new(),
            id: block_id(3),
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    id: operation_id(12),
                    result: OperationResult::Scalar(scalar(12, 8)),
                    kind: OperationKind::ByteSequenceRead {
                        source: place_id(3),
                        index: value_id(20),
                        length: value_id(10),
                        obligation: obligation_id(1),
                    },
                },
                emit_byte(13, 12),
            ],
            terminator: finish(4),
        },
        Block {
            structural_parameters: Vec::new(),
            id: block_id(4),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: finish(5),
        },
    ];
    module.machines.push(helper);
    module
}

pub(super) fn certificate(module: &TerminalModule) -> ProofBundle {
    let sites = terminal_verifier::reconstruct_terminal_obligations(module).unwrap();
    let [site] = sites.obligations() else {
        panic!("one canonical read obligation")
    };
    assert!(site.canonical_certificate);
    assert_eq!(
        site.obligation.class,
        proof_admission::ObligationClass::Derivable
    );
    assert_eq!(
        site.obligation.proposition,
        Proposition::LessThan(
            ScalarTerm::value(value_id(20), unsigned_type(64)),
            ScalarTerm::value(value_id(10), unsigned_type(64)),
        )
    );
    let citation = site
        .semantic_axioms
        .iter()
        .position(|fact| fact == &site.obligation.proposition)
        .expect("selected guard fact");
    let proof = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: site.obligation.id,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: site.obligation.proposition.clone(),
                    rule: ProofRule::SemanticAxiom { index: citation },
                },
            }),
        }],
        ..ProofBundle::default()
    };
    verify_module(module, &proof, &AdmissionProfile::default()).unwrap();
    proof
}

#[test]
fn guarded_byte_reads_preserve_raw_bytes_empty_skip_continuation_and_fuel() {
    for (bytes, byte_index, expected) in [
        (Vec::new(), 0, vec![7]),
        (vec![0, 0x80, 0xff], 0, vec![0, 7]),
        (vec![0, 0x80, 0xff], 1, vec![0x80, 7]),
        (vec![0, 0x80, 0xff], 2, vec![0xff, 7]),
        (vec![0xff], 1, vec![7]),
        (vec![0xff], u64::MAX, vec![7]),
    ] {
        let module = guarded_module(bytes, byte_index);
        let proof = encode_proof_bundle(&certificate(&module)).unwrap();
        let semantic = encode_module(&module).unwrap();
        assert_eq!(decode_module(&semantic).unwrap(), module);
        let mut reference = None;
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
                    TerminalExecutionStatus::Complete(result) => {
                        assert_eq!(result, TerminalExecutionResult::Unit);
                        break;
                    }
                    status => panic!("unexpected guarded read status: {status:?}"),
                }
            }
            let actual = handler
                .effects
                .iter()
                .map(|effect| {
                    let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
                        panic!("boundary")
                    };
                    let [
                        TerminalScalarValue::Integer {
                            scalar_type,
                            value: IntegerValue::Unsigned(byte),
                        },
                    ] = arguments.as_slice()
                    else {
                        panic!("exact unsigned byte")
                    };
                    assert_eq!(ScalarType::Integer(*scalar_type), unsigned_type(8));
                    *byte
                })
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
            assert_eq!(
                meter
                    .usage()
                    .at(FuelChargeSite::Operation(operation_id(12)))
                    .map(|charge| charge.units()),
                (expected.len() == 2).then_some(1)
            );
            if let Some(usage) = &reference {
                assert_eq!(meter.usage(), usage);
            } else {
                reference = Some(meter.usage().clone());
            }
        }
    }
}

#[test]
fn byte_read_rejects_duplicate_obligations_and_guard_facts_from_other_paths() {
    let base = guarded_module(vec![0xff], 0);
    let proof = certificate(&base);
    let mut duplicate = base.clone();
    let mut second = read(&mut duplicate).clone();
    second.id = operation_id(14);
    second.result = OperationResult::Scalar(scalar(14, 8));
    duplicate.machines[1].blocks[1].operations.insert(1, second);
    assert!(matches!(
        terminal_verifier::validate_module_representation(&duplicate),
        Err(ModuleError::DuplicateObligation(_))
    ));

    let mut false_arm = base.clone();
    false_arm.machines[1].blocks[2].operations =
        std::mem::take(&mut false_arm.machines[1].blocks[1].operations);
    terminal_verifier::validate_module_representation(&false_arm).unwrap();
    assert!(matches!(
        verify_module(&false_arm, &proof, &AdmissionProfile::default()),
        Err(VerificationError::RejectedEvidence { .. })
    ));

    let mut joined = base;
    let operations = std::mem::take(&mut joined.machines[1].blocks[1].operations);
    for (position, edge) in [(1, 4), (2, 5)] {
        joined.machines[1].blocks[position].terminator = Terminator::Jump {
            structural_arguments: Vec::new(),
            edge: edge_id(edge),
            target: block_id(5),
            arguments: Vec::new(),
            residual_affine_discards: Vec::new(),
            trivial_affine_discards: Vec::new(),
        };
    }
    joined.machines[1].blocks.push(Block {
        structural_parameters: Vec::new(),
        id: block_id(5),
        parameters: Vec::new(),
        operations,
        terminator: finish(6),
    });
    terminal_verifier::validate_module_representation(&joined).unwrap();
    assert!(matches!(
        verify_module(&joined, &proof, &AdmissionProfile::default()),
        Err(VerificationError::RejectedEvidence { .. })
    ));
}

fn read(module: &mut TerminalModule) -> &mut Operation {
    &mut module.machines[1].blocks[1].operations[0]
}

#[test]
fn byte_read_rejects_fake_wrong_source_later_and_sibling_length() {
    let base = guarded_module(vec![0xff], 0);
    for placement in 0..4 {
        let mut module = base.clone();
        let mut length = Operation {
            id: operation_id(14),
            result: OperationResult::Scalar(scalar(14, 64)),
            kind: OperationKind::ByteSequenceLength {
                source: place_id(3),
            },
        };
        if placement == 0 {
            length = integer(14, 64, 100);
        }
        if let OperationKind::ByteSequenceRead { length, .. } = &mut read(&mut module).kind {
            *length = value_id(14);
        }
        match placement {
            0 => module.machines[1].blocks[0].operations.insert(0, length),
            1 => module.machines[1].blocks[1].operations.push(length),
            2 => module.machines[1].blocks[2].operations.push(length),
            3 => {
                let helper = &mut module.machines[1];
                let mut parameter = helper.structural_parameters[0].clone();
                parameter.place = place_id(6);
                parameter.position = 1;
                helper.structural_parameters.push(parameter);
                helper.structural_places.push(StructuralPlaceDeclaration {
                    id: place_id(6),
                    kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                        position: 1,
                        is_self: false,
                    },
                });
                length.kind = OperationKind::ByteSequenceLength {
                    source: place_id(6),
                };
                helper.blocks[0].operations.insert(0, length);
                if let OperationKind::CallUnit {
                    structural_arguments,
                    ..
                } = &mut module.machines[0].blocks[0].operations[2].kind
                {
                    structural_arguments.push(structural_arguments[0].clone());
                }
            }
            _ => unreachable!(),
        }
        let error = terminal_verifier::validate_module_representation(&module).unwrap_err();
        if placement == 0 || placement == 3 {
            assert!(
                matches!(error, ModuleError::InvalidByteSequenceReadLength { .. }),
                "{error:?}"
            );
        }
        assert!(encode_module(&module).is_err());
    }
    for length in [20, 999] {
        let mut module = base.clone();
        if let OperationKind::ByteSequenceRead {
            length: operand, ..
        } = &mut read(&mut module).kind
        {
            *operand = value_id(length);
        }
        assert!(matches!(
            terminal_verifier::validate_module_representation(&module),
            Err(ModuleError::InvalidByteSequenceReadLength { .. })
        ));
    }
}

#[test]
fn byte_read_rejects_wrong_result_index_and_view_custody() {
    let base = guarded_module(vec![0xff], 0);
    for scalar_type in [
        ScalarType::Boolean,
        unsigned_type(16),
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 8).unwrap()),
    ] {
        let mut module = base.clone();
        read(&mut module).result = OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(12),
            scalar_type,
        });
        assert!(matches!(
            terminal_verifier::validate_module_representation(&module),
            Err(ModuleError::ByteSequenceReadRequiresU8Result(_))
        ));
    }
    for bits in [8, 32, 128] {
        let mut module = base.clone();
        module.machines[1].blocks[0]
            .operations
            .insert(0, integer(14, bits, 0));
        if let OperationKind::ByteSequenceRead { index, .. } = &mut read(&mut module).kind {
            *index = value_id(14);
        }
        assert!(matches!(
            terminal_verifier::validate_module_representation(&module),
            Err(ModuleError::ByteSequenceReadOperandTypeMismatch { .. })
        ));
    }
    let mut signed = base.clone();
    let mut index = integer(14, 64, 0);
    index.result = OperationResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(14),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap()),
    });
    index.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Signed(-1),
    };
    signed.machines[1].blocks[0].operations.insert(0, index);
    if let OperationKind::ByteSequenceRead { index, .. } = &mut read(&mut signed).kind {
        *index = value_id(14);
    }
    assert!(matches!(
        terminal_verifier::validate_module_representation(&signed),
        Err(ModuleError::ByteSequenceReadOperandTypeMismatch { .. })
    ));
    for access in [
        StructuralAccess::Owned,
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut module = base.clone();
        module.machines[1].structural_parameters[0].access = access;
        assert!(terminal_verifier::validate_module_representation(&module).is_err());
    }
}

#[test]
fn byte_read_requires_exact_selected_guard_and_certificate() {
    let base = guarded_module(vec![0xff], 0);
    let proof = certificate(&base);
    assert!(matches!(
        verify_module(&base, &ProofBundle::default(), &AdmissionProfile::default()),
        Err(VerificationError::MissingEvidence(_))
    ));
    let mut retargeted = proof.clone();
    retargeted.evidence[0].obligation = obligation_id(2);
    assert!(verify_module(&base, &retargeted, &AdmissionProfile::default()).is_err());
    for mutation in 0..4 {
        let mut module = base.clone();
        match mutation {
            0 => {
                module.machines[1].blocks[0].terminator = Terminator::Jump {
                    structural_arguments: Vec::new(),
                    edge: edge_id(2),
                    target: block_id(3),
                    arguments: Vec::new(),
                    residual_affine_discards: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                };
                module.machines[1].blocks.pop();
            }
            1 => {
                module.machines[1].blocks[0].operations[1].kind =
                    OperationKind::IntegerLessOrEqual {
                        left: value_id(20),
                        right: value_id(10),
                    };
            }
            2 => {
                if let OperationKind::ByteSequenceRead { index, .. } = &mut read(&mut module).kind {
                    *index = value_id(10);
                }
            }
            3 => {
                module.machines[1].blocks[0].operations.insert(
                    1,
                    Operation {
                        id: operation_id(14),
                        result: OperationResult::Scalar(scalar(14, 64)),
                        kind: OperationKind::ByteSequenceLength {
                            source: place_id(3),
                        },
                    },
                );
                if let OperationKind::ByteSequenceRead { length, .. } = &mut read(&mut module).kind
                {
                    *length = value_id(14);
                }
            }
            _ => unreachable!(),
        }
        terminal_verifier::validate_module_representation(&module).unwrap();
        assert!(
            matches!(
                verify_module(&module, &proof, &AdmissionProfile::default()),
                Err(VerificationError::RejectedEvidence { .. })
            ),
            "mutation {mutation}"
        );
    }
}

#[test]
fn byte_read_wire_rejects_tampered_operands_and_stale_vocabulary() {
    let module = guarded_module(vec![0xff], 0);
    let semantic = encode_module(&module).unwrap();
    assert_eq!(&semantic[10..12], &99_u16.to_le_bytes());
    for generation in [90_u16, 91, 92, 93, 94, 95, 96, 97, 98, 100] {
        let mut stale = semantic.clone();
        stale[10..12].copy_from_slice(&generation.to_le_bytes());
        assert!(decode_module(&stale).is_err());
    }
    let mut marker = vec![56];
    for identity in [3_u64, 20, 10, 1] {
        marker.extend(identity.to_le_bytes());
    }
    let sites = semantic
        .windows(marker.len())
        .enumerate()
        .filter_map(|(position, bytes)| (bytes == marker).then_some(position))
        .collect::<Vec<_>>();
    let [position] = sites.as_slice() else {
        panic!("unique encoded read")
    };
    for offset in [1, 9, 17] {
        let mut tampered = semantic.clone();
        tampered[position + offset..position + offset + 8].copy_from_slice(&999_u64.to_le_bytes());
        assert!(decode_module(&tampered).is_err());
    }
}
