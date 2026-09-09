use super::*;
use terminal_interpreter::{TerminalScalarArrayResult, TerminalScalarArrayValue};

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
fn scalar_array_structural_argument_transport_rejects_before_execution() {
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
    let error = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect_err("unsupported array transport rejects");
    assert!(
        matches!(error,
            VerificationError::Module(ModuleError::ScalarArrayResultMismatch(operation))
            | VerificationError::Module(ModuleError::UnknownStructuralArgument { operation, .. })
                if operation == operation_id(100)
        ),
        "{error:?}"
    );
}
