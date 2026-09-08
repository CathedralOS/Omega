use super::*;

#[test]
fn cyclic_mutating_callee_preserves_projected_receiver_and_every_fuel_pause() {
    check_cyclic_receiver(true);
}

#[test]
fn cyclic_mutating_callee_preserves_whole_receiver_and_every_fuel_pause() {
    check_cyclic_receiver(false);
}

fn check_cyclic_receiver(projected: bool) {
    let module = cyclic_receiver_module(projected);
    let semantic = encode_module(&module).expect("cyclic receiver encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
    let execute = || {
        TerminalExecution::start_artifact_with_structural_arguments(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[],
            &[TerminalStructuralValue {
                opaque_identity: 95,
                structural_type: structural_type_id(if projected { 95 } else { 96 }),
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
        )
        .expect("unranked mutable receiver verifies without a termination claim")
    };
    let mut uninterrupted = execute();
    let mut meter = TerminalFuelMeter::with_allowance(1000);
    let expected = TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
        TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
            value: IntegerValue::Signed(123),
        },
    ));
    assert_eq!(uninterrupted.resume(&mut meter).unwrap(), expected);
    let total = meter.usage().total_units();
    // This is an observed test budget, not a claimed static work certificate.
    for allowance in 0..total {
        let mut execution = execute();
        let mut meter = TerminalFuelMeter::with_allowance(allowance);
        assert!(matches!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        meter.replenish(total - allowance).unwrap();
        assert_eq!(execution.resume(&mut meter).unwrap(), expected);
        assert_eq!(meter.usage().total_units(), total);
        for operation in [23, 27, 45] {
            assert_eq!(
                meter
                    .usage()
                    .at(FuelChargeSite::Operation(operation_id(operation)))
                    .unwrap()
                    .executions(),
                4,
                "each nested call and store commits once per iteration",
            );
        }
    }
}

fn cyclic_receiver_module(projected: bool) -> TerminalModule {
    let mut module = structural_scalar_field_call_module();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[1].shape else {
        unreachable!()
    };
    let mut total = fields[0].clone();
    total.id = structural_field_id(2);
    total.identity = "total".into();
    fields.push(total);
    let integer = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    let scalar = |identity, kind| Operation {
        id: operation_id(identity),
        result: OperationResult::Scalar(ValueDeclaration {
            id: value_id(identity),
            scalar_type: integer,
        }),
        kind,
    };
    let constant = |identity, value| {
        scalar(
            identity,
            OperationKind::IntegerConstant {
                value: IntegerValue::Signed(value),
            },
        )
    };
    let read = |identity, source, field| {
        scalar(
            identity,
            OperationKind::IntegerStructuralField {
                source: place_id(source),
                field: structural_field_id(field),
            },
        )
    };
    let store = |identity, destination, field, value| Operation {
        id: operation_id(identity),
        result: OperationResult::Unit,
        kind: OperationKind::StructuralScalarFieldStore {
            destination: place_id(destination),
            path: Vec::new(),
            field: structural_field_id(field),
            value: value_id(value),
        },
    };
    let call = |identity, callee, receiver| Operation {
        id: operation_id(identity),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: machine_id(callee),
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: place_id(receiver),
                path: Vec::new(),
                access: StructuralAccess::MutableBorrow,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    };
    let jump = |identity, target| Terminator::Jump {
        edge: edge_id(identity),
        target: block_id(target),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    let block = |identity, operations, terminator| Block {
        id: block_id(identity),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations,
        terminator,
    };
    let successor = |identity, target| SuccessorEdge {
        edge: edge_id(identity),
        target: block_id(target),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };

    // The outer caller projects its item exactly once and observes its writes
    // after the entire cyclic invocation returns.
    let mut reader = module.machines[1].clone();
    reader.id = machine_id(98);
    reader.contract = empty_contract(contract_id(98));
    reader.entry = block_id(101);
    reader.blocks[0].id = block_id(101);
    reader.structural_parameters[0].place = place_id(98);
    reader.structural_places[0].id = place_id(98);
    reader.blocks[0].operations[0] = read(50, 98, 2);
    let Terminator::Return { value, .. } = &mut reader.blocks[0].terminator else {
        unreachable!()
    };
    *value = value_id(50);
    let caller = &mut module.machines[0];
    caller.blocks[0].operations[0] = constant(1, 0);
    let mut initialize_total = caller.blocks[0].operations[1].clone();
    initialize_total.id = operation_id(4);
    let OperationKind::StructuralScalarFieldStore { field, .. } = &mut initialize_total.kind else {
        unreachable!()
    };
    *field = structural_field_id(2);
    let mut invoke = call(3, 96, 95);
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut invoke.kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::Field("item".into())];
    let mut observe = caller.blocks[0].operations.pop().unwrap();
    observe.id = operation_id(5);
    let OperationKind::CallStructuralScalar { callee, .. } = &mut observe.kind else {
        unreachable!()
    };
    *callee = machine_id(98);
    caller.blocks[0]
        .operations
        .extend([initialize_total, invoke, observe]);

    let receiver = &mut module.machines[1];
    receiver.result = TerminalMachineResult::Unit;
    receiver.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    receiver.blocks = vec![
        block(96, Vec::new(), jump(20, 97)),
        block(
            97,
            vec![
                read(20, 96, 1),
                constant(21, 4),
                Operation {
                    id: operation_id(22),
                    result: OperationResult::Scalar(ValueDeclaration {
                        id: value_id(22),
                        scalar_type: ScalarType::Boolean,
                    }),
                    kind: OperationKind::IntegerLessThan {
                        left: value_id(20),
                        right: value_id(21),
                    },
                },
            ],
            Terminator::Conditional {
                condition: value_id(22),
                when_true: successor(21, 98),
                when_false: successor(22, 99),
            },
        ),
        block(
            98,
            vec![
                call(23, 97, 96),
                read(24, 96, 1),
                constant(25, 1),
                scalar(
                    26,
                    OperationKind::WrappingIntegerAdd {
                        left: value_id(24),
                        right: value_id(25),
                    },
                ),
                store(27, 96, 1, 26),
            ],
            jump(23, 97),
        ),
        block(
            99,
            Vec::new(),
            Terminator::ReturnUnit {
                edge: edge_id(24),
                trivial_affine_discards: Vec::new(),
            },
        ),
    ];
    let mut recorder = receiver.clone();
    recorder.id = machine_id(97);
    recorder.contract = empty_contract(contract_id(97));
    recorder.entry = block_id(100);
    recorder.structural_parameters[0].place = place_id(97);
    recorder.structural_places[0].id = place_id(97);
    recorder.blocks = vec![block(
        100,
        vec![
            read(40, 97, 2),
            constant(41, 10),
            scalar(
                42,
                OperationKind::WrappingIntegerMultiply {
                    left: value_id(40),
                    right: value_id(41),
                },
            ),
            read(43, 97, 1),
            scalar(
                44,
                OperationKind::WrappingIntegerAdd {
                    left: value_id(42),
                    right: value_id(43),
                },
            ),
            store(45, 97, 2, 44),
        ],
        Terminator::ReturnUnit {
            edge: edge_id(40),
            trivial_affine_discards: Vec::new(),
        },
    )];
    module.machines.push(recorder);
    module.machines.push(reader);
    if !projected {
        let caller = &mut module.machines[0];
        caller.attachment = Some(structural_type_id(96));
        caller.structural_parameters[0].structural_type = structural_type_id(96);
        for operation in &mut caller.blocks[0].operations {
            match &mut operation.kind {
                OperationKind::StructuralScalarFieldStore { path, .. } => path.clear(),
                OperationKind::CallUnit {
                    structural_arguments,
                    ..
                }
                | OperationKind::CallStructuralScalar {
                    structural_arguments,
                    ..
                } => {
                    structural_arguments[0].path.clear();
                }
                _ => {}
            }
        }
    }
    module
}
