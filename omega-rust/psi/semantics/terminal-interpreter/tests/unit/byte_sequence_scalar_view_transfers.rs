//! Scalar-return calls consume the selected descriptor, not its former source.
use super::byte_sequence_read::{integer, scalar, successor};
use super::byte_sequence_scalar_calls::module;
use super::byte_sequence_subslice::certificate;
use super::*;

fn borrowed(place: u64) -> StructuralArgument {
    StructuralArgument {
        place: place_id(place),
        path: Vec::new(),
        access: StructuralAccess::SharedBorrow,
    }
}

fn observe(module: &TerminalModule, expected: u128) {
    let semantic = encode_module(module).unwrap();
    let proof = encode_proof_bundle(&certificate(module)).unwrap();
    assert_eq!(decode_module(&semantic).unwrap(), *module);
    let mut reference = None;
    for incremental in [false, true] {
        let mut execution =
            TerminalExecution::start_artifact(&semantic, &proof, &AdmissionProfile::default(), &[])
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
                    assert_eq!(
                        result,
                        TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                            scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                            value: IntegerValue::Unsigned(expected),
                        })
                    );
                    break;
                }
                status => panic!("unexpected view-transfer status: {status:?}"),
            }
        }
        assert!(handler.effects.is_empty());
        if let Some(usage) = &reference {
            assert_eq!(meter.usage(), usage);
        } else {
            reference = Some(meter.usage().clone());
        }
    }
}

fn subslice_module(bytes: Vec<u8>, start: u64) -> TerminalModule {
    let mut source = module(bytes, start, true);
    let caller = &mut source.machines[2];
    let mut calls = std::mem::take(&mut caller.blocks[0].operations);
    for call in &mut calls {
        let OperationKind::CallStructuralScalar {
            arguments,
            structural_arguments,
            ..
        } = &mut call.kind
        else {
            unreachable!();
        };
        *arguments = vec![value_id(63)];
        *structural_arguments = vec![borrowed(8)];
    }
    let returned = caller.blocks[0].terminator.clone();
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(8),
        kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
            producer: operation_id(62),
            structural_type: structural_type_id(1),
        },
    });
    caller.blocks[0].operations.extend([
        Operation {
            id: operation_id(60),
            result: OperationResult::Scalar(scalar(60, 64)),
            kind: OperationKind::ByteSequenceLength {
                source: place_id(4),
            },
        },
        Operation {
            id: operation_id(61),
            result: OperationResult::Scalar(ValueDeclaration {
                id: value_id(61),
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::IntegerLessOrEqual {
                left: value_id(40),
                right: value_id(60),
            },
        },
    ]);
    caller.blocks[0].terminator = Terminator::Conditional {
        condition: value_id(61),
        when_true: successor(8, 8),
        when_false: successor(9, 9),
    };
    let mut operations = vec![
        Operation {
            id: operation_id(62),
            result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                place: place_id(8),
                structural_type: structural_type_id(1),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::ByteSequenceSubslice {
                source: place_id(4),
                start: value_id(40),
                end: value_id(60),
                length: value_id(60),
                obligation: obligation_id(2),
            },
        },
        integer(63, 64, 0),
    ];
    operations.extend(calls);
    caller.blocks.extend([
        Block {
            id: block_id(8),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations,
            terminator: returned,
        },
        Block {
            id: block_id(9),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![integer(64, 64, 257)],
            terminator: Terminator::Return {
                edge: edge_id(10),
                value: value_id(64),
                cleanup_actions: Vec::new(),
            },
        },
    ]);
    source
}

#[test]
fn subslice_scalar_calls_observe_windows_and_only_selected_arrivals() {
    for (bytes, start, expected) in [
        (vec![0, 0x80, 0xff], 1, 0x80),
        (vec![0, 0x80, 0xff], 2, 0xff),
        (vec![0, 0x80, 0xff], 3, 256),
        (vec![], 0, 256),
        (vec![], 1, 257),
        (vec![0xff], u64::MAX, 257),
    ] {
        observe(&subslice_module(bytes, start), expected);
    }
}

fn block_module(select_first: bool, byte_index: u64) -> TerminalModule {
    let mut source = module(vec![0, 0x80, 0xff], byte_index, true);
    let mut parameter = source.machines[1].structural_parameters[0].clone();
    parameter.place = place_id(9);
    let root = &mut source.machines[0];
    root.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(8),
        kind: semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 1,
            structural_type: structural_type_id(1),
        },
    });
    root.blocks[0].operations.insert(
        1,
        Operation {
            id: operation_id(50),
            result: OperationResult::Unit,
            kind: OperationKind::EstablishByteSequenceLiteral {
                destination: place_id(8),
                bytes: vec![0x17, 0xfe],
            },
        },
    );
    root.blocks[0].operations.insert(
        3,
        Operation {
            id: operation_id(51),
            result: OperationResult::Scalar(ValueDeclaration {
                id: value_id(51),
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanConstant {
                value: select_first,
            },
        },
    );
    for operation in &mut root.blocks[0].operations[4..] {
        let OperationKind::CallStructuralScalar {
            arguments,
            structural_arguments,
            ..
        } = &mut operation.kind
        else {
            unreachable!();
        };
        arguments.push(value_id(51));
        structural_arguments.push(borrowed(8));
    }
    let caller = &mut source.machines[2];
    caller.parameters.push(ValueDeclaration {
        id: value_id(49),
        scalar_type: ScalarType::Boolean,
    });
    let mut second = caller.structural_parameters[0].clone();
    second.place = place_id(7);
    second.position = 1;
    caller.structural_parameters.push(second);
    let mut calls = std::mem::take(&mut caller.blocks[0].operations);
    for call in &mut calls {
        let OperationKind::CallStructuralScalar {
            structural_arguments,
            ..
        } = &mut call.kind
        else {
            unreachable!();
        };
        *structural_arguments = vec![borrowed(9)];
    }
    let returned = caller.blocks[0].terminator.clone();
    caller.structural_places.extend([
        StructuralPlaceDeclaration {
            id: place_id(7),
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(9),
            kind: semantic_vocabulary::StructuralPlaceKind::BlockParameter {
                block: block_id(8),
                position: 0,
            },
        },
    ]);
    let mut when_true = successor(8, 8);
    when_true.structural_arguments = vec![borrowed(4)];
    let mut when_false = successor(9, 8);
    when_false.structural_arguments = vec![borrowed(7)];
    caller.blocks[0].terminator = Terminator::Conditional {
        condition: value_id(49),
        when_true,
        when_false,
    };
    caller.blocks.push(Block {
        id: block_id(8),
        parameters: Vec::new(),
        structural_parameters: vec![parameter],
        operations: calls,
        terminator: returned,
    });
    source
}

#[test]
fn block_parameter_scalar_calls_observe_the_selected_literal() {
    for (selected, expected) in [
        (true, vec![0, 0x80, 0xff, 256]),
        (false, vec![0x17, 0xfe, 256, 256]),
    ] {
        for (byte_index, byte) in expected.into_iter().enumerate() {
            observe(&block_module(selected, byte_index as u64), byte);
        }
    }
}

#[test]
fn scalar_calls_reject_future_and_sibling_view_producers() {
    for sibling in [false, true] {
        let mut source = subslice_module(vec![0, 0x80], 1);
        let helper = &mut source.machines[2];
        let producer = helper.blocks[1].operations.remove(0);
        if sibling {
            helper.blocks[2].operations.insert(0, producer);
        } else {
            helper.blocks[1].operations.push(producer);
        }
        assert_eq!(
            terminal_verifier::validate_module_representation(&source),
            Err(ModuleError::ByteSequenceViewNotEstablished {
                operation: operation_id(43),
                place: place_id(8),
            })
        );
    }
}

#[test]
fn scalar_call_cannot_read_a_sibling_blocks_view_parameter() {
    let mut source = block_module(false, 0);
    let helper = &mut source.machines[2];
    let Terminator::Conditional { when_false, .. } = &mut helper.blocks[0].terminator else {
        unreachable!();
    };
    when_false.target = block_id(9);
    when_false.structural_arguments.clear();
    let mut sibling = helper.blocks[1].clone();
    sibling.id = block_id(9);
    sibling.structural_parameters.clear();
    for (position, operation) in sibling.operations.iter_mut().enumerate() {
        let ordinal = 70 + position as u64;
        operation.id = operation_id(ordinal);
        operation.result = OperationResult::Scalar(scalar(ordinal, 64));
    }
    sibling.terminator = Terminator::Return {
        edge: edge_id(11),
        value: value_id(71),
        cleanup_actions: Vec::new(),
    };
    helper.blocks.push(sibling);
    assert_eq!(
        terminal_verifier::validate_module_representation(&source),
        Err(ModuleError::ByteSequenceViewNotEstablished {
            operation: operation_id(70),
            place: place_id(9),
        })
    );
}
