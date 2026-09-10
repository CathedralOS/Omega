use super::*;

#[path = "arguments/selection.rs"]
mod selection;

fn parameters(machine: &mut TerminalMachine, count: u32) {
    for position in 0..count {
        let place = place_id(machine.id.get() * 1000 + u64::from(position) + 1);
        machine
            .structural_parameters
            .push(StructuralParameterDeclaration {
                place,
                position,
                is_self: false,
                structural_type: structural_type_id(1),
                access: StructuralAccess::Owned,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: vec![],
                projected_qualifications: vec![],
            });
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                position,
                is_self: false,
            },
        });
    }
}

fn callee(identity: u64) -> TerminalMachine {
    let mut machine = unit_module().machines.remove(0);
    machine.id = machine_id(identity);
    machine.contract.id = contract_id(identity);
    machine.entry = block_id(identity);
    machine.blocks[0].id = block_id(identity);
    machine.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(identity),
        trivial_affine_discards: vec![],
    };
    parameters(&mut machine, 2);
    machine
}

fn actual(place: PlaceId) -> StructuralArgument {
    StructuralArgument {
        place,
        path: vec![],
        access: StructuralAccess::Owned,
    }
}

fn call_result(
    machine: &mut TerminalMachine,
    operation: u64,
    place: u64,
) -> StructuralOperationResult {
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(place),
        kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
            producer: operation_id(operation),
            structural_type: structural_type_id(1),
        },
    });
    StructuralOperationResult {
        place: place_id(place),
        structural_type: structural_type_id(1),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: vec![],
        projected_qualifications: vec![],
        claims: vec![],
    }
}

fn return_array(machine: &mut TerminalMachine, source: PlaceId) {
    let place = place_id(machine.id.get() * 1000 + 99);
    machine.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place,
        structural_type: structural_type_id(1),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: vec![],
        projected_qualifications: vec![],
    });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place,
        kind: semantic_vocabulary::StructuralPlaceKind::Result,
    });
    machine.blocks[0].terminator = Terminator::ReturnStructural {
        edge: edge_id(machine.id.get()),
        source,
        returned_claims: vec![],
        trivial_affine_discards: vec![],
    };
}

fn module(
    dimensions: &[u64],
    duplicate: bool,
    prior: bool,
    leaves: &[TerminalScalarValue],
) -> TerminalModule {
    let scalar_type = leaves
        .first()
        .map_or(byte(0).scalar_type(), |value| value.scalar_type());
    let mut module = fixture(dimensions, scalar_type, leaves);
    let entry = &mut module.machines[0];
    let second = call_result(entry, 100, 3);
    entry.blocks[0].operations.push(Operation {
        id: operation_id(100),
        result: OperationResult::Structural(second),
        kind: OperationKind::EstablishScalarArray {
            elements: (1..=leaves.len())
                .rev()
                .map(|position| value_id(position as u64))
                .collect(),
        },
    });
    let arguments = vec![
        actual(place_id(3)),
        actual(place_id(if duplicate { 3 } else { 1 })),
    ];
    entry.blocks[0].operations.push(Operation {
        id: operation_id(101),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: machine_id(2),
            arguments: vec![],
            structural_arguments: arguments.clone(),
            claim_transfers: vec![],
            requirement_obligations: vec![],
            crash_continuations: vec![],
        },
    });
    entry.blocks[0].operations.push(Operation {
        id: operation_id(102),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(102),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::CallStructuralScalar {
            callee: machine_id(3),
            arguments: vec![],
            structural_arguments: arguments.clone(),
            claim_transfers: vec![],
            requirement_obligations: vec![],
            crash_continuations: vec![],
        },
    });
    let result = call_result(entry, 103, 4);
    entry.blocks[0].operations.push(Operation {
        id: operation_id(103),
        result: OperationResult::Structural(result),
        kind: OperationKind::CallStructuralWithScalarArguments {
            callee: machine_id(4),
            arguments: vec![],
            structural_arguments: arguments,
            claim_transfers: vec![],
            returned_claim_transfers: vec![],
            requirement_obligations: vec![],
            crash_continuations: vec![],
        },
    });
    let Terminator::ReturnStructural { source, .. } = &mut entry.blocks[0].terminator else {
        unreachable!()
    };
    *source = place_id(if prior { 1 } else { 4 });

    let sink = callee(2);
    let mut scalar = callee(3);
    scalar.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(3002),
        scalar_type: ScalarType::Boolean,
    });
    scalar.blocks[0].operations.push(Operation {
        id: operation_id(3001),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(3001),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::BooleanConstant { value: true },
    });
    scalar.blocks[0].terminator = Terminator::Return {
        edge: edge_id(3),
        value: value_id(3001),
        cleanup_actions: vec![],
    };
    let mut forwarding = callee(4);
    let result = call_result(&mut forwarding, 4001, 4003);
    forwarding.blocks[0].operations.push(Operation {
        id: operation_id(4001),
        result: OperationResult::Structural(result),
        kind: OperationKind::CallStructural {
            callee: machine_id(5),
            structural_arguments: vec![actual(place_id(4002)), actual(place_id(4001))],
            claim_transfers: vec![],
            returned_claim_transfers: vec![],
            requirement_obligations: vec![],
            crash_continuations: vec![],
            selected_evidence: vec![],
        },
    });
    return_array(&mut forwarding, place_id(4003));
    let mut identity = callee(5);
    return_array(&mut identity, place_id(5001));
    module.machines.extend([sink, scalar, forwarding, identity]);
    module
}

#[test]
fn array_arguments_preserve_order_duplicates_forwarding_and_caller_payload_at_every_fuel_pause() {
    for dimensions in [vec![2], vec![2, 2], vec![0], vec![1, 0], vec![0, 2]] {
        let leaves = (0..dimensions.iter().product::<u64>())
            .map(|position| byte(7 + position as u8))
            .collect::<Vec<_>>();
        for duplicate in [false, true] {
            for prior in [false, true] {
                let module = module(&dimensions, duplicate, prior, &leaves);
                let semantic = encode_module(&module).unwrap();
                assert_eq!(decode_module(&semantic).unwrap(), module);
                let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
                let mut expected_leaves = leaves.clone();
                if duplicate && !prior {
                    expected_leaves.reverse();
                }
                let expected = expected(expected_leaves);
                let measured = interpret_terminal_artifact_measured(
                    &semantic,
                    &proof,
                    &AdmissionProfile::default(),
                    &[],
                )
                .unwrap();
                assert_eq!(measured.value(), expected);
                let mut execution = TerminalExecution::start_artifact(
                    &semantic,
                    &proof,
                    &AdmissionProfile::default(),
                    &[],
                )
                .unwrap();
                let mut meter = TerminalFuelMeter::with_allowance(0);
                for _ in 0..measured.usage().total_units() {
                    assert!(matches!(
                        execution.resume(&mut meter).unwrap(),
                        TerminalExecutionStatus::SponsorExhausted(_)
                    ));
                    meter.replenish(1).unwrap();
                }
                assert_eq!(
                    execution.resume(&mut meter).unwrap(),
                    TerminalExecutionStatus::Complete(expected)
                );
                assert_eq!(meter.usage(), measured.usage());
            }
        }
    }
}

#[test]
fn array_arguments_preserve_boolean_and_ieee_bit_patterns() {
    for leaves in [
        vec![
            TerminalScalarValue::Boolean(true),
            TerminalScalarValue::Boolean(false),
        ],
        vec![
            TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(0x8000_0000)),
            TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(0x7fc0_0042)),
        ],
    ] {
        let module = module(&[2], true, false, &leaves);
        let result = interpret_terminal_artifact_measured(
            &encode_module(&module).unwrap(),
            &encode_proof_bundle(&ProofBundle::default()).unwrap(),
            &AdmissionProfile::default(),
            &[],
        )
        .unwrap();
        assert_eq!(result.value(), expected(leaves.into_iter().rev().collect()));
    }
}

#[test]
fn opaque_host_array_parameters_cannot_invent_returned_contents() {
    for dimensions in [vec![2], vec![0], vec![1, 0]] {
        let mut module = fixture(&dimensions, byte(0).scalar_type(), &[]);
        let mut identity = callee(1);
        return_array(&mut identity, place_id(1001));
        module.machines = vec![identity];
        let arguments = [100, 101].map(|opaque_identity| TerminalStructuralValue {
            opaque_identity,
            structural_type: structural_type_id(1),
            qualifications: vec![],
            path: vec![],
        });
        let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
            &encode_module(&module).unwrap(),
            &encode_proof_bundle(&ProofBundle::default()).unwrap(),
            &AdmissionProfile::default(),
            &[],
            &arguments,
        )
        .expect("opaque entry arguments do not manufacture typed payloads");
        assert!(matches!(
            execution.resume(&mut TerminalFuelMeter::unbounded()),
            Err(TerminalInterpretError::VerifiedOperationMalformed)
        ));
    }
}
