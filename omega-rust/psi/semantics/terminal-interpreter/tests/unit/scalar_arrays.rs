use super::*;
use terminal_interpreter::{TerminalScalarArrayResult, TerminalScalarArrayValue};

#[path = "scalar_arrays/arguments.rs"]
mod arguments;

fn byte(value: u8) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        value: IntegerValue::Unsigned(value.into()),
    }
}

fn fixture(
    dimensions: &[u64],
    scalar_type: ScalarType,
    values: &[TerminalScalarValue],
) -> TerminalModule {
    let mut module = unit_module();
    for (dimension, length) in dimensions.iter().enumerate() {
        module.structural_types.push(StructuralTypeDeclaration {
            id: structural_type_id(dimension as u64 + 1),
            identity: format!("array_{dimension}"),
            shape: StructuralTypeShape::FixedArray {
                element: structural_type_id(dimension as u64 + 2),
                length: *length,
            },
        });
    }
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(dimensions.len() as u64 + 1),
        identity: "element".into(),
        shape: StructuralTypeShape::PrimitiveScalar(scalar_type),
    });
    let machine = &mut module.machines[0];
    machine.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: place_id(2),
        structural_type: structural_type_id(1),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: vec![],
        projected_qualifications: vec![],
    });
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: place_id(1),
            kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer: operation_id(99),
                structural_type: structural_type_id(1),
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(2),
            kind: semantic_vocabulary::StructuralPlaceKind::Result,
        },
    ];
    for (position, value) in values.iter().enumerate() {
        let kind = match value {
            TerminalScalarValue::Integer { value, .. } => {
                OperationKind::IntegerConstant { value: *value }
            }
            TerminalScalarValue::Boolean(value) => OperationKind::BooleanConstant { value: *value },
            TerminalScalarValue::IeeeFloat(value) => {
                OperationKind::IeeeFloatConstant { value: *value }
            }
        };
        machine.blocks[0].operations.push(Operation {
            id: operation_id(position as u64 + 1),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(position as u64 + 1),
                scalar_type: value.scalar_type(),
            }),
            kind,
        });
    }
    machine.blocks[0].operations.push(Operation {
        id: operation_id(99),
        result: OperationResult::Structural(StructuralOperationResult {
            place: place_id(1),
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: vec![],
            projected_qualifications: vec![],
            claims: vec![],
        }),
        kind: OperationKind::EstablishScalarArray {
            elements: (1..=values.len())
                .map(|position| value_id(position as u64))
                .collect(),
        },
    });
    machine.blocks[0].terminator = Terminator::ReturnStructural {
        edge: edge_id(1),
        source: place_id(1),
        returned_claims: vec![],
        trivial_affine_discards: vec![],
    };
    module
}

fn expected(values: Vec<TerminalScalarValue>) -> TerminalExecutionResult {
    TerminalExecutionResult::ScalarArray(TerminalScalarArrayResult {
        value: TerminalScalarArrayValue {
            structural_type: structural_type_id(1),
            elements: values,
        },
    })
}

#[test]
fn scalar_arrays_decode_and_return_exact_primitive_and_nested_empty_contents() {
    let cases = [
        (vec![2], byte(0).scalar_type(), vec![byte(7), byte(9)]),
        (
            vec![2, 2],
            ScalarType::Boolean,
            vec![
                TerminalScalarValue::Boolean(true),
                TerminalScalarValue::Boolean(false),
                TerminalScalarValue::Boolean(false),
                TerminalScalarValue::Boolean(true),
            ],
        ),
        (vec![0], byte(0).scalar_type(), vec![]),
        (vec![1, 0], byte(0).scalar_type(), vec![]),
        (vec![0, 2], byte(0).scalar_type(), vec![]),
        (vec![u64::MAX, u64::MAX, 0], byte(0).scalar_type(), vec![]),
        (
            vec![2],
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
            vec![
                TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(0x8000_0000)),
                TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(0x7fc0_0042)),
            ],
        ),
    ];
    let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
    for (dimensions, scalar_type, values) in cases {
        let module = fixture(&dimensions, scalar_type, &values);
        let semantic = encode_module(&module).unwrap();
        assert_eq!(decode_module(&semantic).unwrap(), module);
        let measured = interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[],
        )
        .unwrap();
        assert_eq!(measured.value(), expected(values.clone()));
        assert_eq!(measured.usage().total_units(), values.len() as u64 + 2);
        assert!(decode_module(&semantic[..semantic.len() - 1]).is_err());
    }
}

#[test]
fn scalar_array_fuel_pauses_before_establishment_and_return_without_replaying() {
    for values in [vec![byte(7), byte(9)], vec![]] {
        let module = fixture(&[values.len() as u64], byte(0).scalar_type(), &values);
        let semantic = encode_module(&module).unwrap();
        let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
        let mut execution =
            TerminalExecution::start_artifact(&semantic, &proof, &AdmissionProfile::default(), &[])
                .unwrap();
        let mut meter = TerminalFuelMeter::with_allowance(values.len() as u64);
        assert!(matches!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        assert!(
            meter
                .usage()
                .at(FuelChargeSite::Operation(operation_id(99)))
                .is_none()
        );
        meter.replenish(1).unwrap();
        assert!(matches!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        assert_eq!(
            meter
                .usage()
                .at(FuelChargeSite::Operation(operation_id(99)))
                .unwrap()
                .executions(),
            1
        );
        meter.replenish(1).unwrap();
        assert_eq!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::Complete(expected(values.clone()))
        );
        assert_eq!(meter.usage().total_units(), values.len() as u64 + 2);
        assert_eq!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::Complete(expected(values.clone()))
        );
        assert_eq!(meter.usage().total_units(), values.len() as u64 + 2);
    }
}

#[test]
fn scalar_array_operand_order_changes_canonical_identity_and_actual_contents() {
    let module = fixture(&[2], byte(0).scalar_type(), &[byte(7), byte(9)]);
    let mut changed = module.clone();
    let OperationKind::EstablishScalarArray { elements } = &mut changed.machines[0].blocks[0]
        .operations
        .last_mut()
        .unwrap()
        .kind
    else {
        panic!("array constructor");
    };
    elements.swap(0, 1);
    assert_ne!(
        terminal_codec::semantic_fingerprint(&module).unwrap(),
        terminal_codec::semantic_fingerprint(&changed).unwrap()
    );
    let measured = interpret_terminal_artifact_measured(
        &encode_module(&changed).unwrap(),
        &encode_proof_bundle(&ProofBundle::default()).unwrap(),
        &AdmissionProfile::default(),
        &[],
    )
    .unwrap();
    assert_eq!(measured.value(), expected(vec![byte(9), byte(7)]));
}

#[test]
fn scalar_array_forged_shapes_operands_custody_and_consumers_reject() {
    let original = fixture(&[2], byte(0).scalar_type(), &[byte(7), byte(9)]);
    for mutation in 0..9 {
        let mut module = original.clone();
        let machine = &mut module.machines[0];
        let operation = machine.blocks[0].operations.last_mut().unwrap();
        match mutation {
            0 => {
                if let OperationKind::EstablishScalarArray { elements } = &mut operation.kind {
                    elements.pop();
                }
            }
            1 => {
                if let OperationKind::EstablishScalarArray { elements } = &mut operation.kind {
                    elements[0] = value_id(88);
                }
            }
            2 => {
                if let OperationResult::Structural(result) = &mut operation.result {
                    result.multiplicity = StructuralMultiplicity::Affine;
                }
            }
            3 => {
                machine.structural_places[0].kind =
                    semantic_vocabulary::StructuralPlaceKind::OperationResult {
                        producer: operation_id(88),
                        structural_type: structural_type_id(1),
                    }
            }
            4 => {
                module.structural_types[1].shape =
                    StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean)
            }
            5 => {
                module.structural_types[1].shape = StructuralTypeShape::FixedArray {
                    element: structural_type_id(1),
                    length: 0,
                }
            }
            6 => module.structural_types[1].shape = StructuralTypeShape::Record { fields: vec![] },
            7 => {
                machine.blocks[0].terminator = Terminator::ReturnStructural {
                    edge: edge_id(1),
                    source: place_id(2),
                    returned_claims: vec![],
                    trivial_affine_discards: vec![],
                }
            }
            8 => {
                let late = machine.blocks[0].operations.remove(0);
                machine.blocks[0].operations.push(late);
            }
            _ => unreachable!(),
        }
        assert!(
            verify_module(
                &module,
                &ProofBundle::default(),
                &AdmissionProfile::default()
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
    for dimensions in [vec![u64::MAX, 2], vec![0]] {
        let mut module = fixture(&dimensions, byte(0).scalar_type(), &[]);
        if dimensions == [0] {
            module.structural_types.last_mut().unwrap().shape =
                StructuralTypeShape::Record { fields: vec![] };
        }
        assert!(
            verify_module(
                &module,
                &ProofBundle::default(),
                &AdmissionProfile::default()
            )
            .is_err()
        );
    }
}

#[test]
fn scalar_array_contents_survive_later_scalar_work_and_nested_local_arrays() {
    let mut module = fixture(&[2], byte(0).scalar_type(), &[byte(7), byte(9)]);
    let mut callee = fixture(&[2], byte(0).scalar_type(), &[byte(11), byte(13)])
        .machines
        .remove(0);
    callee.id = machine_id(2);
    callee.contract.id = contract_id(2);
    callee.entry = block_id(2);
    callee.blocks[0].id = block_id(2);
    callee.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(10),
        scalar_type: byte(0).scalar_type(),
    });
    callee.structural_places.pop();
    callee.structural_places[0].id = place_id(11);
    callee.structural_places[0].kind = semantic_vocabulary::StructuralPlaceKind::OperationResult {
        producer: operation_id(199),
        structural_type: structural_type_id(1),
    };
    for operation in &mut callee.blocks[0].operations {
        operation.id = operation_id(operation.id.get() + 100);
        if let OperationResult::Scalar(value) = &mut operation.result {
            value.id = value_id(value.id.get() + 10);
        }
        if let OperationResult::Structural(result) = &mut operation.result {
            result.place = place_id(11);
        }
        if let OperationKind::EstablishScalarArray { elements } = &mut operation.kind {
            for element in elements {
                *element = value_id(element.get() + 10);
            }
        }
    }
    callee.blocks[0].terminator = Terminator::Return {
        edge: edge_id(2),
        value: value_id(11),
        cleanup_actions: vec![],
    };
    module.machines.push(callee);
    module.machines[0].blocks[0].operations.extend([
        Operation {
            id: operation_id(100),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(3),
                scalar_type: byte(0).scalar_type(),
            }),
            kind: OperationKind::Call {
                callee: machine_id(2),
                arguments: vec![],
                requirement_obligations: vec![],
                crash_continuations: vec![],
            },
        },
        Operation {
            id: operation_id(200),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(4),
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanConstant { value: true },
        },
    ]);
    let semantic = encode_module(&module).unwrap();
    let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
    let mut execution =
        TerminalExecution::start_artifact(&semantic, &proof, &AdmissionProfile::default(), &[])
            .unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(7);
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(3).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(expected(vec![byte(7), byte(9)]))
    );
    assert_eq!(meter.usage().total_units(), 10);
}

#[test]
fn scalar_array_owned_unit_argument_preserves_caller_contents() {
    let mut module = fixture(&[2], byte(0).scalar_type(), &[byte(7), byte(9)]);
    let mut callee = unit_module().machines.remove(0);
    callee.id = machine_id(2);
    callee.contract.id = contract_id(2);
    callee.entry = block_id(2);
    callee.blocks[0].id = block_id(2);
    callee.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(2),
        trivial_affine_discards: vec![],
    };
    callee
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: place_id(11),
            position: 0,
            is_self: false,
            structural_type: structural_type_id(1),
            access: StructuralAccess::Owned,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: vec![],
            projected_qualifications: vec![],
        });
    callee.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(11),
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    });
    module.machines.push(callee);
    module.machines[0].blocks[0].operations.push(Operation {
        id: operation_id(100),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: machine_id(2),
            arguments: vec![],
            structural_arguments: vec![StructuralArgument {
                place: place_id(1),
                path: vec![],
                access: StructuralAccess::Owned,
            }],
            claim_transfers: vec![],
            requirement_obligations: vec![],
            crash_continuations: vec![],
        },
    });
    assert_eq!(
        interpret_terminal_artifact_measured(
            &encode_module(&module).unwrap(),
            &encode_proof_bundle(&ProofBundle::default()).unwrap(),
            &AdmissionProfile::default(),
            &[],
        )
        .unwrap()
        .value(),
        expected(vec![byte(7), byte(9)]),
    );
}

fn internal_array_returns(dimensions: &[u64], return_prior_array: bool) -> TerminalModule {
    let count = dimensions.iter().product::<u64>();
    let mut module = fixture(dimensions, byte(0).scalar_type(), &[]);
    module.machines.clear();
    for depth in 0..3_u64 {
        let values = vec![byte(7 + depth as u8); count as usize];
        let mut machine = fixture(dimensions, byte(0).scalar_type(), &values)
            .machines
            .remove(0);
        let offset = depth * 1000;
        machine.id = machine_id(depth + 1);
        machine.contract.id = contract_id(depth + 1);
        machine.entry = block_id(depth + 1);
        machine.blocks[0].id = block_id(depth + 1);
        let TerminalMachineResult::Structural(result) = &mut machine.result else {
            unreachable!()
        };
        result.place = place_id(offset + 2);
        machine.structural_places[0].id = place_id(offset + 1);
        machine.structural_places[0].kind =
            semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer: operation_id(offset + 99),
                structural_type: structural_type_id(1),
            };
        machine.structural_places[1].id = place_id(offset + 2);
        for operation in &mut machine.blocks[0].operations {
            operation.id = operation_id(offset + operation.id.get());
            match &mut operation.result {
                OperationResult::Scalar(value) => value.id = value_id(offset + value.id.get()),
                OperationResult::Structural(result) => result.place = place_id(offset + 1),
                OperationResult::Unit => unreachable!(),
            }
            if let OperationKind::EstablishScalarArray { elements } = &mut operation.kind {
                for element in elements {
                    *element = value_id(offset + element.get());
                }
            }
        }
        let mut source = place_id(offset + 1);
        if depth < 2 {
            machine.structural_places.push(StructuralPlaceDeclaration {
                id: place_id(offset + 3),
                kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                    producer: operation_id(offset + 100),
                    structural_type: structural_type_id(1),
                },
            });
            machine.blocks[0].operations.push(Operation {
                id: operation_id(offset + 100),
                result: OperationResult::Structural(StructuralOperationResult {
                    place: place_id(offset + 3),
                    structural_type: structural_type_id(1),
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    qualifications: vec![],
                    projected_qualifications: vec![],
                    claims: vec![],
                }),
                kind: OperationKind::CallStructural {
                    callee: machine_id(depth + 2),
                    structural_arguments: vec![],
                    claim_transfers: vec![],
                    returned_claim_transfers: vec![],
                    requirement_obligations: vec![],
                    crash_continuations: vec![],
                    selected_evidence: vec![],
                },
            });
            machine.blocks[0].operations.push(Operation {
                id: operation_id(offset + 101),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: value_id(offset + 101),
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanConstant { value: true },
            });
            if !return_prior_array {
                source = place_id(offset + 3);
            }
        }
        machine.blocks[0].terminator = Terminator::ReturnStructural {
            edge: edge_id(depth + 1),
            source,
            returned_claims: vec![],
            trivial_affine_discards: vec![],
        };
        module.machines.push(machine);
    }
    module
}

#[test]
fn scalar_array_internal_returns_preserve_nested_payloads_and_caller_arrays_across_fuel() {
    let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
    for dimensions in [vec![2], vec![2, 2], vec![0], vec![1, 0], vec![0, 2]] {
        for return_prior_array in [false, true] {
            let module = internal_array_returns(&dimensions, return_prior_array);
            let semantic = encode_module(&module).unwrap();
            assert_eq!(decode_module(&semantic).unwrap(), module);
            let count = dimensions.iter().product::<u64>();
            let result = expected(vec![
                byte(if return_prior_array { 7 } else { 9 });
                count as usize
            ]);
            let measured = interpret_terminal_artifact_measured(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                &[],
            )
            .unwrap();
            assert_eq!(measured.value(), result);
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
                TerminalExecutionStatus::Complete(result.clone())
            );
            assert_eq!(meter.usage(), measured.usage());
            assert_eq!(
                execution.resume(&mut meter).unwrap(),
                TerminalExecutionStatus::Complete(result)
            );
            assert_eq!(meter.usage(), measured.usage());
        }
    }
}

#[test]
fn scalar_array_internal_returns_reject_forged_result_and_call_evidence() {
    for mutation in 0..4 {
        let mut module = internal_array_returns(&[2], false);
        let caller = &mut module.machines[0];
        let operation = &mut caller.blocks[0].operations[3];
        match mutation {
            0 => {
                let OperationResult::Structural(result) = &mut operation.result else {
                    unreachable!()
                };
                result.structural_type = structural_type_id(2);
            }
            1 => {
                let OperationResult::Structural(result) = &mut operation.result else {
                    unreachable!()
                };
                result.multiplicity = StructuralMultiplicity::Affine;
            }
            2 => {
                let OperationKind::CallStructural {
                    requirement_obligations,
                    ..
                } = &mut operation.kind
                else {
                    unreachable!()
                };
                requirement_obligations.push(obligation_id(1));
            }
            3 => {
                caller.structural_places[2].kind =
                    semantic_vocabulary::StructuralPlaceKind::OperationResult {
                        producer: operation_id(99),
                        structural_type: structural_type_id(1),
                    };
            }
            _ => unreachable!(),
        }
        assert!(
            verify_module(
                &module,
                &ProofBundle::default(),
                &AdmissionProfile::default()
            )
            .is_err(),
            "forged internal array result {mutation}"
        );
        // Some contradictions are already rejected by canonical encoding.
        // Encodable contradictions must still fail admission before execution.
        if let Ok(semantic) = encode_module(&module) {
            assert!(
                TerminalExecution::start_artifact(
                    &semantic,
                    &encode_proof_bundle(&ProofBundle::default()).unwrap(),
                    &AdmissionProfile::default(),
                    &[],
                )
                .is_err(),
                "forged internal array artifact {mutation}"
            );
        }
    }
}

#[test]
fn scalar_array_internal_returns_preserve_boolean_and_ieee_payload_bits() {
    for payload in [
        TerminalScalarValue::Boolean(true),
        TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(0x8000_0000)),
        TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(0x7fc0_0042)),
    ] {
        let mut module = internal_array_returns(&[2], false);
        module.structural_types.last_mut().unwrap().shape =
            StructuralTypeShape::PrimitiveScalar(payload.scalar_type());
        for machine in &mut module.machines {
            for operation in &mut machine.blocks[0].operations {
                if !matches!(operation.kind, OperationKind::IntegerConstant { .. }) {
                    continue;
                }
                let OperationResult::Scalar(result) = &mut operation.result else {
                    panic!("literal result");
                };
                result.scalar_type = payload.scalar_type();
                operation.kind = match payload {
                    TerminalScalarValue::Boolean(value) => OperationKind::BooleanConstant { value },
                    TerminalScalarValue::IeeeFloat(value) => {
                        OperationKind::IeeeFloatConstant { value }
                    }
                    _ => panic!("noninteger payload"),
                };
            }
        }
        assert_eq!(
            interpret_terminal_artifact_measured(
                &encode_module(&module).unwrap(),
                &encode_proof_bundle(&ProofBundle::default()).unwrap(),
                &AdmissionProfile::default(),
                &[],
            )
            .unwrap()
            .value(),
            expected(vec![payload; 2])
        );
    }
}

#[test]
fn scalar_array_call_results_do_not_erase_callee_requirements() {
    let mut module = internal_array_returns(&[2], false);
    let mut proof = ProofBundle::default();
    for machine in &mut module.machines {
        machine.contract.requires.push(Proposition::Truth);
        for operation in &mut machine.blocks[0].operations {
            if let OperationKind::CallStructural {
                requirement_obligations,
                ..
            } = &mut operation.kind
            {
                let identity = proof.evidence.len() as u64 + 1;
                requirement_obligations.push(obligation_id(identity));
                proof.evidence.push(ObligationEvidence {
                    obligation: obligation_id(identity),
                    route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                        identity: EvidenceIdentity::new(identity).unwrap(),
                        proof_system_marker: ProofSystemMarker::CURRENT,
                        proof: ProofNode {
                            conclusion: Proposition::Truth,
                            rule: ProofRule::Assumption { index: 0 },
                        },
                    }),
                });
            }
        }
    }
    let semantics = encode_module(&module).unwrap();
    assert_eq!(
        interpret_terminal_artifact_measured(
            &semantics,
            &encode_proof_bundle(&proof).unwrap(),
            &AdmissionProfile::default(),
            &[],
        )
        .unwrap()
        .value(),
        expected(vec![byte(9); 2])
    );
    assert!(
        TerminalExecution::start_artifact(
            &semantics,
            &encode_proof_bundle(&ProofBundle::default()).unwrap(),
            &AdmissionProfile::default(),
            &[],
        )
        .is_err()
    );
    let OperationKind::CallStructural {
        requirement_obligations,
        ..
    } = &mut module.machines[0].blocks[0].operations[3].kind
    else {
        panic!("caller operation");
    };
    requirement_obligations.clear();
    assert!(verify_module(&module, &proof, &AdmissionProfile::default()).is_err());
}

#[test]
fn scalar_array_call_results_keep_payload_through_unit_arguments() {
    let mut module = internal_array_returns(&[2], false);
    let mut sink = unit_module().machines.remove(0);
    sink.id = machine_id(4);
    sink.contract.id = contract_id(4);
    sink.entry = block_id(4);
    sink.blocks[0].id = block_id(4);
    sink.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(4),
        trivial_affine_discards: vec![],
    };
    sink.structural_parameters
        .push(StructuralParameterDeclaration {
            place: place_id(301),
            position: 0,
            is_self: false,
            structural_type: structural_type_id(1),
            access: StructuralAccess::Owned,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: vec![],
            projected_qualifications: vec![],
        });
    sink.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(301),
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    });
    module.machines.push(sink);
    module.machines[0].blocks[0].operations.push(Operation {
        id: operation_id(302),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: machine_id(4),
            arguments: vec![],
            structural_arguments: vec![StructuralArgument {
                place: place_id(3),
                path: vec![],
                access: StructuralAccess::Owned,
            }],
            claim_transfers: vec![],
            requirement_obligations: vec![],
            crash_continuations: vec![],
        },
    });
    assert_eq!(
        interpret_terminal_artifact_measured(
            &encode_module(&module).unwrap(),
            &encode_proof_bundle(&ProofBundle::default()).unwrap(),
            &AdmissionProfile::default(),
            &[],
        )
        .unwrap()
        .value(),
        expected(vec![byte(9), byte(9)]),
    );
}
