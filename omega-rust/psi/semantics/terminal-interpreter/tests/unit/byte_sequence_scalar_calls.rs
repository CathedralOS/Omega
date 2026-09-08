//! Scalar results remain observable across literal-backed, repeated and nested calls.
use super::byte_sequence_read::{certificate, guarded_module, integer, scalar};
use super::*;

pub(super) fn module(bytes: Vec<u8>, byte_index: u64, nested: bool) -> TerminalModule {
    let mut module = guarded_module(bytes, byte_index);
    let reader = &mut module.machines[1];
    reader.result = TerminalMachineResult::Scalar(scalar(29, 64));
    reader.blocks[1].operations.truncate(1);
    reader.blocks[1].operations.push(Operation {
        id: operation_id(16),
        result: OperationResult::Scalar(scalar(16, 64)),
        kind: OperationKind::IntegerWiden {
            operand: value_id(12),
        },
    });
    reader.blocks[1].terminator = Terminator::Return {
        edge: edge_id(4),
        value: value_id(16),
        cleanup_actions: Vec::new(),
    };
    reader.blocks[2].operations = vec![integer(17, 64, 256)];
    reader.blocks[2].terminator = Terminator::Return {
        edge: edge_id(5),
        value: value_id(17),
        cleanup_actions: Vec::new(),
    };
    let call = |ordinal, callee, argument, place| Operation {
        id: operation_id(ordinal),
        result: OperationResult::Scalar(scalar(ordinal, 64)),
        kind: OperationKind::CallStructuralScalar {
            callee: machine_id(callee),
            arguments: vec![value_id(argument)],
            structural_arguments: vec![StructuralArgument {
                place: place_id(place),
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    };
    let mut wrapper = reader.clone();
    wrapper.id = machine_id(3);
    wrapper.contract.id = contract_id(3);
    wrapper.entry = block_id(6);
    wrapper.parameters = vec![scalar(40, 64)];
    wrapper.structural_parameters[0].place = place_id(4);
    wrapper.structural_places[0].id = place_id(4);
    wrapper.result = TerminalMachineResult::Scalar(scalar(45, 64));
    wrapper.blocks.truncate(1);
    wrapper.blocks[0].id = wrapper.entry;
    wrapper.blocks[0].operations = vec![call(43, 2, 40, 4), call(44, 2, 40, 4)];
    wrapper.blocks[0].terminator = Terminator::Return {
        edge: edge_id(6),
        value: value_id(44),
        cleanup_actions: Vec::new(),
    };
    let caller = &mut module.machines[0];
    caller.result = TerminalMachineResult::Scalar(scalar(32, 64));
    caller.blocks[0].operations.truncate(2);
    caller.blocks[0].operations.extend([
        call(30, if nested { 3 } else { 2 }, 2, 1),
        call(31, if nested { 3 } else { 2 }, 2, 1),
    ]);
    caller.blocks[0].terminator = Terminator::Return {
        edge: edge_id(1),
        value: value_id(31),
        cleanup_actions: Vec::new(),
    };
    if nested {
        module.machines.push(wrapper);
    }
    module
}

#[test]
fn literal_backed_scalar_calls_return_exact_bytes_across_nested_calls_and_suspension() {
    for nested in [false, true] {
        for (bytes, byte_index, expected) in [
            (vec![], 0, 256),
            (vec![0, 0x80, 0xff], 0, 0),
            (vec![0, 0x80, 0xff], 1, 0x80),
            (vec![0, 0x80, 0xff], 2, 0xff),
            (vec![0xff], 1, 256),
            (vec![0xff], u64::MAX, 256),
        ] {
            let module = module(bytes, byte_index, nested);
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
                            assert_eq!(
                                result,
                                TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 64)
                                        .unwrap(),
                                    value: IntegerValue::Unsigned(expected),
                                })
                            );
                            break;
                        }
                        status => panic!("unexpected scalar byte-call status: {status:?}"),
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
    }
}
