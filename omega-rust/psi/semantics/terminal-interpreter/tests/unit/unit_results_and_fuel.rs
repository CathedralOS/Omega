use super::{
    RecordingHandler, artifact_sections, assert_write_only_store_atomic, block_id, claim_id,
    contract_id, edge_id, effect_module, empty_contract, internal_structural_call_module,
    joined_parameter_dynamic_scalar_call_module, machine_id,
    multi_claim_internal_structural_call_module, nearest_fma_module, operation_id,
    parameter_dynamic_scalar_call_module, payloadless_call_module, payloadless_case_module,
    place_id, rebound_dynamic_scalar_call_module, reference_release_module, structural_case_id,
    structural_domain_id, structural_scalar_field_call_module, structural_type_id,
    structural_value, value_id, write_only_boolean_call_module, write_only_primitive_call_module,
};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    IeeeFloatFormat, IeeeFloatValue, IntegerSign, IntegerType, IntegerValue, ScalarType,
};
use terminal_codec::{decode_module, encode_module, encode_proof_section};
use terminal_fuel::{FuelChargeSite, FuelExhaustion, TerminalFuelMeter, TerminalFuelSchedule};
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalInterpretError,
    TerminalScalarCaseResult, TerminalScalarCaseValue, TerminalScalarValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue,
    interpret_terminal_artifact_measured,
};
use terminal_psi::{
    Block, EntryClaim, Operation, OperationKind, OperationResult, StructuralAccess,
    StructuralArgument, StructuralDomainDeclaration, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralResultDeclaration, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{ModuleError, ProofBundle, VerificationError, verify_module};

#[test]
fn unit_artifact_interprets_as_a_value_less_normal_result() {
    let (semantic, proof) = artifact_sections();
    let measured = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
    )
    .expect("unit artifact should interpret");

    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), 1);
    assert_eq!(
        measured
            .usage()
            .at(FuelChargeSite::Edge(edge_id(1)))
            .unwrap()
            .units(),
        1
    );
}

#[test]
fn nearest_ieee_fma_executes_one_rounding_for_both_interchange_formats() {
    let cases = [
        (
            [
                IeeeFloatValue::Binary32(0x3f80_0001),
                IeeeFloatValue::Binary32(0x3f7f_fffe),
                IeeeFloatValue::Binary32(0xbf80_0000),
            ],
            IeeeFloatValue::Binary32(0xa880_0000),
        ),
        (
            [
                IeeeFloatValue::Binary64(0x3ff0_0000_0000_0001),
                IeeeFloatValue::Binary64(0x3fef_ffff_ffff_fffe),
                IeeeFloatValue::Binary64(0xbff0_0000_0000_0000),
            ],
            IeeeFloatValue::Binary64(0xb970_0000_0000_0000),
        ),
    ];

    for (operands, expected) in cases {
        let module = nearest_fma_module(operands);
        let semantic = encode_module(&module).expect("nearest-FMA semantics encode");
        let proof =
            encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
        let measured = interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs::default(),
            &mut AcceptTerminalEffects,
        )
        .expect("verified nearest-FMA executes");

        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::IeeeFloat(expected)),
        );
        assert_ne!(
            expected,
            match expected {
                IeeeFloatValue::Binary32(_) => IeeeFloatValue::Binary32(0),
                IeeeFloatValue::Binary64(_) => IeeeFloatValue::Binary64(0),
            }
        );
        assert_eq!(measured.usage().total_units(), 5);
    }
}

#[test]
fn verifier_rejects_nearest_fma_with_a_mixed_format_operand() {
    let mut module = nearest_fma_module([
        IeeeFloatValue::Binary32(0x3f80_0000),
        IeeeFloatValue::Binary32(0x4000_0000),
        IeeeFloatValue::Binary32(0x4040_0000),
    ]);
    let wrong = &mut module.machines[0].blocks[0].operations[1];
    wrong.result = OperationResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(2),
        scalar_type: ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
    });
    wrong.kind = OperationKind::IeeeFloatConstant {
        value: IeeeFloatValue::Binary64(0x4000_0000_0000_0000),
    };

    let Err(error) = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    ) else {
        panic!("mixed-format nearest-FMA must reject")
    };
    assert_eq!(
        error,
        VerificationError::Module(ModuleError::IeeeFloatFusedMultiplyAddOperandTypeMismatch {
            operation: operation_id(4),
            operand: value_id(2),
            expected: ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
            actual: ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
        }),
    );
}

#[test]
fn payloadless_case_construction_returns_exact_case_and_costs_one_operation() {
    let structural_type = structural_type_id(1);
    let result_case = structural_case_id(1);
    let module = payloadless_case_module();

    let semantic = encode_module(&module).expect("payloadless case semantics encode");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let measured = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
    )
    .expect("verified payloadless case executes");

    assert_eq!(
        measured.value(),
        TerminalExecutionResult::ScalarCase(TerminalScalarCaseResult {
            value: TerminalScalarCaseValue {
                structural_type,
                result_case,
                fields: vec![],
            },
        })
    );
    assert_eq!(measured.usage().total_units(), 2);
    assert_eq!(
        measured
            .usage()
            .at(FuelChargeSite::Operation(operation_id(1)))
            .unwrap()
            .units(),
        1
    );
}

#[test]
fn payloadless_structural_call_returns_exact_case_in_four_resumable_units() {
    let module = payloadless_call_module();
    let semantic = encode_module(&module).expect("payloadless call semantics encode");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .expect("verified payloadless call starts");
    let mut meter = TerminalFuelMeter::with_allowance(3);

    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            site: FuelChargeSite::Edge(edge_id(1)),
            required_units: 1,
            remaining_units: 0,
        })
    );
    assert_eq!(meter.usage().total_units(), 3);
    meter.replenish(1).unwrap();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::ScalarCase(
            TerminalScalarCaseResult {
                value: TerminalScalarCaseValue {
                    structural_type: structural_type_id(1),
                    result_case: structural_case_id(1),
                    fields: vec![],
                },
            }
        ))
    );
    assert_eq!(meter.usage().total_units(), 4);
    assert_eq!(
        meter
            .usage()
            .at(FuelChargeSite::Operation(operation_id(1)))
            .unwrap()
            .units(),
        1,
        "the callee constructor is not replayed after caller-edge exhaustion"
    );
}

#[test]
fn unit_return_fuel_exhaustion_resumes_without_advancing_or_double_charging() {
    let (semantic, proof) = artifact_sections();
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .expect("unit artifact should start");
    let mut meter = TerminalFuelMeter::with_allowance(0);

    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            site: FuelChargeSite::Edge(edge_id(1)),
            required_units: 1,
            remaining_units: 0,
        })
    );
    assert_eq!(meter.usage().total_units(), 0);

    meter.replenish(1).unwrap();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meter.usage().total_units(), 1);
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meter.usage().total_units(), 1);
}

#[test]
fn write_only_primitive_store_survives_unit_call_and_is_atomic_at_fuel_exhaustion() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    assert_write_only_store_atomic(
        write_only_primitive_call_module(),
        91,
        TerminalScalarValue::Integer {
            scalar_type: integer,
            value: IntegerValue::Unsigned(1),
        },
        TerminalScalarValue::Integer {
            scalar_type: integer,
            value: IntegerValue::Unsigned(7),
        },
    );
}

#[test]
fn write_only_boolean_store_survives_unit_call_and_is_atomic_at_fuel_exhaustion() {
    assert_write_only_store_atomic(
        write_only_boolean_call_module(),
        93,
        TerminalScalarValue::Boolean(false),
        TerminalScalarValue::Boolean(true),
    );
}

#[test]
fn reference_release_preserves_backing_and_is_atomic_at_fuel_exhaustion() {
    let semantic = encode_module(&reference_release_module()).unwrap();
    let proof = encode_proof_section(&reference_release_module(), &ProofBundle::default()).unwrap();
    let initial = TerminalStructuralPrimitiveValue {
        argument_index: 0,
        value: TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            value: IntegerValue::Unsigned(7),
        },
    };
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 94,
                structural_type: structural_type_id(91),
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            primitive_values: &[initial],
            ..Default::default()
        },
    )
    .expect("verified reference establishment and release start");
    let mut meter = TerminalFuelMeter::with_allowance(1);
    let exhausted = execution
        .resume(&mut meter, &mut AcceptTerminalEffects)
        .unwrap();
    assert!(
        matches!(exhausted, TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
        site: FuelChargeSite::Operation(id), ..
    }) if id == operation_id(95))
    );
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        exhausted
    );
    assert_eq!(execution.structural_primitive_values(), vec![initial]);
    meter.replenish(2).unwrap();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(execution.structural_primitive_values(), vec![initial]);
    assert_eq!(
        meter
            .usage()
            .at(FuelChargeSite::Operation(operation_id(94)))
            .unwrap()
            .executions(),
        1
    );
    assert_eq!(
        meter
            .usage()
            .at(FuelChargeSite::Operation(operation_id(95)))
            .unwrap()
            .executions(),
        1
    );
}

#[test]
fn mutable_reference_temporarily_lends_shared_read_and_write_only_store() {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut module = reference_release_module();
        let mut callee = write_only_primitive_call_module().machines.remove(1);
        callee.structural_parameters[0].access = access;
        if access == StructuralAccess::SharedBorrow {
            let mut read = callee.blocks[0].operations.remove(0);
            read.kind = OperationKind::PrimitiveScalarRead {
                path: Vec::new(),
                source: place_id(92),
            };
            callee.blocks[0].operations = vec![read];
        }
        module.machines.push(callee);
        module.machines[0].blocks[0].operations.insert(
            1,
            Operation {
                static_reach_binding: None,
                id: operation_id(91),
                result: OperationResult::Unit,
                kind: OperationKind::CallUnit {
                    erased_arguments: Vec::new(),
                    callee: machine_id(92),
                    arguments: Vec::new(),
                    structural_arguments: vec![StructuralArgument {
                        place: place_id(94),
                        path: vec![StructuralPathSegment::Referent],
                        access,
                    }],
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            },
        );
        let semantic = encode_module(&module).unwrap();
        let proof = encode_proof_section(&module, &ProofBundle::default()).unwrap();
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let mut execution = TerminalExecution::start_artifact(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs {
                arguments: &[TerminalStructuralValue {
                    opaque_identity: 94,
                    structural_type: structural_type_id(91),
                    qualifications: Vec::new(),
                    path: Vec::new(),
                }],
                primitive_values: &[TerminalStructuralPrimitiveValue {
                    argument_index: 0,
                    value: TerminalScalarValue::Integer {
                        scalar_type: integer,
                        value: IntegerValue::Unsigned(1),
                    },
                }],
                ..Default::default()
            },
        )
        .expect("temporary reference permission attenuation verifies");
        assert_eq!(
            execution
                .resume(
                    &mut TerminalFuelMeter::unbounded(),
                    &mut AcceptTerminalEffects
                )
                .unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert_eq!(
            execution.structural_primitive_values(),
            vec![TerminalStructuralPrimitiveValue {
                argument_index: 0,
                value: TerminalScalarValue::Integer {
                    scalar_type: integer,
                    value: IntegerValue::Unsigned(if access == StructuralAccess::WriteOnlyBorrow {
                        7
                    } else {
                        1
                    }),
                },
            }]
        );
    }
}

#[test]
fn structural_scalar_field_store_is_visible_through_a_projected_call_without_replay() {
    let module = structural_scalar_field_call_module();
    let semantic = encode_module(&module).expect("structural scalar-field semantics encode");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let structural = TerminalStructuralValue {
        opaque_identity: 95,
        structural_type: structural_type_id(95),
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[structural],
            ..Default::default()
        },
    )
    .expect("verified structural scalar-field call starts");
    let mut meter = TerminalFuelMeter::with_allowance(2);

    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            site: FuelChargeSite::Operation(operation_id(3)),
            required_units: 1,
            remaining_units: 0,
        })
    );
    meter.replenish(4).unwrap();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                value: IntegerValue::Signed(99),
            }
        ))
    );
    assert_eq!(meter.usage().total_units(), 6);
    assert_eq!(
        meter
            .usage()
            .at(FuelChargeSite::Operation(operation_id(2)))
            .unwrap()
            .executions(),
        1,
        "the committed field store is not replayed after replenishment",
    );
}

#[test]
fn structural_scalar_call_binds_scalar_and_structural_arguments_together() {
    let mut module = structural_scalar_field_call_module();
    let OperationKind::CallStructuralScalar { arguments, .. } =
        &mut module.machines[0].blocks[0].operations[2].kind
    else {
        unreachable!()
    };
    arguments.push(value_id(1));
    let callee = &mut module.machines[1];
    callee.parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(6),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap()),
    });
    callee.blocks[0].operations.clear();
    callee.blocks[0].terminator = Terminator::Return {
        edge: edge_id(96),
        value: value_id(6),
        cleanup_actions: Vec::new(),
    };

    let semantic = encode_module(&module).expect("mixed structural-scalar semantics encode");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let structural = TerminalStructuralValue {
        opaque_identity: 95,
        structural_type: structural_type_id(95),
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[structural],
            ..Default::default()
        },
    )
    .expect("verified mixed structural-scalar call starts");
    let mut meter = TerminalFuelMeter::with_allowance(8);
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                value: IntegerValue::Signed(99),
            }
        ))
    );
}

#[test]
fn rebound_dynamic_scalar_call_executes_and_composes_its_fixed_fuel_callee() {
    let module = rebound_dynamic_scalar_call_module();
    let proof_bundle = ProofBundle::default();
    let verified = verify_module(&module, &proof_bundle, &AdmissionProfile::default())
        .expect("rebound dynamic scalar module verifies");
    let certificate = terminal_fixed_fuel::derive_fixed_entry_fuel(&verified, module.entry)
        .expect("rebound dynamic scalar module has a fixed bound");
    terminal_fixed_fuel::validate_fixed_entry_fuel(&verified, &certificate)
        .expect("rebound dynamic scalar fixed-fuel certificate replays");

    let semantic = encode_module(&module).expect("rebound dynamic scalar semantics encode");
    let proof = encode_proof_section(&module, &proof_bundle).expect("empty proof encodes");
    let structural = TerminalStructuralValue {
        opaque_identity: 95,
        structural_type: structural_type_id(95),
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[structural],
            ..Default::default()
        },
    )
    .expect("verified rebound dynamic scalar call starts");
    let mut meter = TerminalFuelMeter::unbounded();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                value: IntegerValue::Signed(99),
            },
        )),
    );
    assert_eq!(meter.usage().total_units(), certificate.ceiling_units());
}

#[test]
fn dynamic_descriptor_parameter_crosses_a_real_interpreter_call() {
    let module = parameter_dynamic_scalar_call_module();
    let proof_bundle = ProofBundle::default();
    verify_module(&module, &proof_bundle, &AdmissionProfile::default())
        .expect("dynamic descriptor parameter module verifies");
    let semantic = encode_module(&module).expect("dynamic descriptor parameter semantics encode");
    let proof = encode_proof_section(&module, &proof_bundle).expect("empty proof encodes");
    let structural = TerminalStructuralValue {
        opaque_identity: 95,
        structural_type: structural_type_id(95),
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[structural],
            ..Default::default()
        },
    )
    .expect("verified dynamic descriptor parameter call starts");
    let mut meter = TerminalFuelMeter::unbounded();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                value: IntegerValue::Signed(99),
            },
        )),
    );
}

#[test]
fn dynamic_descriptor_parameter_joins_two_exact_predecessors() {
    let module = joined_parameter_dynamic_scalar_call_module();
    let proof_bundle = ProofBundle::default();
    verify_module(&module, &proof_bundle, &AdmissionProfile::default())
        .expect("two exact descriptor predecessors verify");
    let semantic = encode_module(&module).expect("joined dynamic descriptor semantics encode");
    assert_eq!(
        decode_module(&semantic).expect("joined dynamic descriptor semantics decode"),
        module,
        "canonical Terminal encoding retains both predecessor arguments",
    );
    let proof = encode_proof_section(&module, &proof_bundle).expect("empty proof encodes");

    for (choose_first, expected) in [(true, 99), (false, 41)] {
        let structural = TerminalStructuralValue {
            opaque_identity: 95,
            structural_type: structural_type_id(95),
            qualifications: Vec::new(),
            path: Vec::new(),
        };
        let mut execution = TerminalExecution::start_artifact(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[TerminalScalarValue::Boolean(choose_first)],
            TerminalStructuralInputs {
                arguments: &[structural],
                ..Default::default()
            },
        )
        .expect("verified joined dynamic descriptor call starts");
        let mut meter = TerminalFuelMeter::unbounded();
        assert_eq!(
            execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                    value: IntegerValue::Signed(expected),
                },
            )),
            "the selected predecessor must carry both its referent and private table",
        );
    }
}

#[test]
fn write_only_primitive_storage_requires_an_existing_exact_value_and_never_observes_it() {
    let module = write_only_primitive_call_module();
    let semantic = encode_module(&module).expect("primitive-store semantics encode");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let structural = TerminalStructuralValue {
        opaque_identity: 92,
        structural_type: structural_type_id(91),
        qualifications: Vec::new(),
        path: Vec::new(),
    };

    assert!(matches!(
        TerminalExecution::start_artifact(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs {
                arguments: std::slice::from_ref(&structural),
                ..Default::default()
            }
        ),
        Err(
            terminal_interpreter::TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::StructuralPrimitiveValueCount {
                    expected: 1,
                    actual: 0,
                }
            )
        )
    ));
    assert!(matches!(
        TerminalExecution::start_artifact(&semantic, &proof, &AdmissionProfile::default(), &[], TerminalStructuralInputs { arguments: std::slice::from_ref(&structural), primitive_values: &[TerminalStructuralPrimitiveValue {
                argument_index: 0,
                value: TerminalScalarValue::Boolean(false),
            }], ..Default::default() }),
        Err(
            terminal_interpreter::TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::StructuralPrimitiveValueType {
                    argument_index: 0,
                    expected: ScalarType::Integer(expected),
                    actual: ScalarType::Boolean,
                }
            )
        ) if expected == integer
    ));

    for prior in [1, 250] {
        let mut handler = RecordingHandler::default();
        let measured = interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs {
                arguments: std::slice::from_ref(&structural),
                primitive_values: &[TerminalStructuralPrimitiveValue {
                    argument_index: 0,
                    value: TerminalScalarValue::Integer {
                        scalar_type: integer,
                        value: IntegerValue::Unsigned(prior),
                    },
                }],
                ..Default::default()
            },
            &mut handler,
        )
        .expect("an existing exact primitive value supplies logical storage");
        assert_eq!(measured.usage().total_units(), 5);
        assert_eq!(
            measured.structural_primitive_values(),
            &[TerminalStructuralPrimitiveValue {
                argument_index: 0,
                value: TerminalScalarValue::Integer {
                    scalar_type: integer,
                    value: IntegerValue::Unsigned(7),
                },
            }]
        );
    }
}

#[test]
fn structural_return_transfers_value_and_claim_atomically_after_edge_charge() {
    let structural_type = structural_type_id(1);
    let domain = structural_domain_id(1);
    let source = place_id(1);
    let result_place = place_id(2);
    let claim = claim_id(1);
    let edge = edge_id(1);
    let module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::Resource".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        }],
        structural_domains: vec![StructuralDomainDeclaration {
            id: domain,
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(1)
                .expect("semantic domain identity"),
            identity: "test::Owned".into(),
            carrier: structural_type,
            content_projection: None,
        }],
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: machine_id(1),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: source,
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Linear,
                access: StructuralAccess::Owned,
                qualifications: vec![domain],
                projected_qualifications: Vec::new(),
            }],
            ranked_scc: None,
            result: TerminalMachineResult::Structural(StructuralResultDeclaration {
                reference_sources: Vec::new(),
                place: result_place,
                structural_type,
                multiplicity: StructuralMultiplicity::Linear,
                qualifications: vec![domain],
                projected_qualifications: Vec::new(),
            }),
            structural_places: vec![
                StructuralPlaceDeclaration {
                    id: source,
                    kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                },
                StructuralPlaceDeclaration {
                    id: result_place,
                    kind: semantic_vocabulary::StructuralPlaceKind::Result,
                },
            ],
            entry_claims: vec![EntryClaim {
                claim,
                input: source,
                path: Vec::new(),
            }],
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(1),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: block_id(1),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnStructural {
                    edge,
                    source,
                    returned_claims: vec![claim],
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: empty_contract(contract_id(1)),
        }],
    };
    let semantic = encode_module(&module).expect("structural return encodes");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let argument = TerminalStructuralValue {
        opaque_identity: 0x5eed,
        structural_type,
        qualifications: vec![domain],
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: std::slice::from_ref(&argument),
            ..Default::default()
        },
    )
    .expect("verified structural return starts");
    let mut meter = TerminalFuelMeter::with_allowance(0);

    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            site: FuelChargeSite::Edge(edge),
            required_units: 1,
            remaining_units: 0,
        })
    );
    assert_eq!(
        execution.live_claim_frontier().collect::<Vec<_>>(),
        vec![claim]
    );
    meter.replenish(1).unwrap();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(
            terminal_interpreter::TerminalStructuralResult {
                value: argument,
                claims: vec![claim],
            }
        ))
    );
    assert!(execution.live_claim_frontier().next().is_none());
    assert_eq!(meter.usage().total_units(), 1);
}

#[test]
fn internal_structural_call_rebinds_claim_and_preserves_value_identity() {
    let module = internal_structural_call_module(false);
    let semantic = encode_module(&module).expect("structural call encodes");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let argument = TerminalStructuralValue {
        opaque_identity: 0xc011,
        structural_type: structural_type_id(1),
        qualifications: vec![structural_domain_id(1)],
        path: Vec::new(),
    };

    let measured = terminal_interpreter::interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: std::slice::from_ref(&argument),
            ..Default::default()
        },
        &mut terminal_interpreter::AcceptTerminalEffects,
    )
    .expect("whole-root structural call should interpret");

    assert_eq!(
        measured.value(),
        TerminalExecutionResult::Structural(terminal_interpreter::TerminalStructuralResult {
            value: argument,
            claims: vec![claim_id(1)],
        })
    );
    assert_eq!(measured.usage().total_units(), 3);
}

#[test]
fn internal_structural_call_resumes_at_each_charge_without_replaying_custody() {
    let module = internal_structural_call_module(false);
    let semantic = encode_module(&module).expect("structural call encodes");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let argument = TerminalStructuralValue {
        opaque_identity: 0xc012,
        structural_type: structural_type_id(1),
        qualifications: vec![structural_domain_id(1)],
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: std::slice::from_ref(&argument),
            ..Default::default()
        },
    )
    .expect("verified structural call starts");
    let mut meter = TerminalFuelMeter::with_allowance(0);

    assert!(matches!(
        execution.resume(&mut meter, &mut AcceptTerminalEffects).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            site: FuelChargeSite::Operation(operation),
            ..
        }) if operation == operation_id(1)
    ));
    assert_eq!(
        execution.live_claim_frontier().collect::<Vec<_>>(),
        vec![claim_id(1)]
    );
    assert_eq!(meter.usage().total_units(), 0);

    meter.replenish(1).unwrap();
    assert!(matches!(
        execution.resume(&mut meter, &mut AcceptTerminalEffects).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            site: FuelChargeSite::Edge(edge),
            ..
        }) if edge == edge_id(2)
    ));
    assert_eq!(
        execution.live_claim_frontier().collect::<Vec<_>>(),
        vec![claim_id(1)]
    );
    assert_eq!(meter.usage().total_units(), 1);

    meter.replenish(1).unwrap();
    assert!(matches!(
        execution.resume(&mut meter, &mut AcceptTerminalEffects).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            site: FuelChargeSite::Edge(edge),
            ..
        }) if edge == edge_id(1)
    ));
    assert_eq!(
        execution.live_claim_frontier().collect::<Vec<_>>(),
        vec![claim_id(1)]
    );
    assert_eq!(meter.usage().total_units(), 2);

    meter.replenish(1).unwrap();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(
            terminal_interpreter::TerminalStructuralResult {
                value: argument,
                claims: vec![claim_id(1)],
            }
        ))
    );
    assert!(execution.live_claim_frontier().next().is_none());
    assert_eq!(meter.usage().total_units(), 3);
}

#[test]
fn internal_multi_claim_structural_call_resumes_without_replaying_or_swapping_claims() {
    let module = multi_claim_internal_structural_call_module(false);
    let semantic = encode_module(&module).expect("multi-claim structural call encodes");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let argument = TerminalStructuralValue {
        opaque_identity: 0xc014,
        structural_type: structural_type_id(1),
        qualifications: vec![structural_domain_id(1)],
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: std::slice::from_ref(&argument),
            ..Default::default()
        },
    )
    .expect("verified multi-claim structural call starts");
    let mut meter = TerminalFuelMeter::with_allowance(0);

    for (expected_site, expected_usage) in [
        (FuelChargeSite::Operation(operation_id(1)), 0),
        (FuelChargeSite::Edge(edge_id(2)), 1),
        (FuelChargeSite::Edge(edge_id(1)), 2),
    ] {
        assert!(matches!(
            execution.resume(&mut meter, &mut AcceptTerminalEffects).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(FuelExhaustion { site, .. })
                if site == expected_site
        ));
        assert_eq!(
            execution.live_claim_frontier().collect::<Vec<_>>(),
            vec![claim_id(1), claim_id(2)]
        );
        assert_eq!(meter.usage().total_units(), expected_usage);
        meter.replenish(1).unwrap();
    }

    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(
            terminal_interpreter::TerminalStructuralResult {
                value: argument,
                claims: vec![claim_id(1), claim_id(2)],
            }
        ))
    );
    assert!(execution.live_claim_frontier().next().is_none());
    assert_eq!(meter.usage().total_units(), 3);
}

#[test]
fn crashing_structural_callee_never_produces_a_caller_result() {
    let module = internal_structural_call_module(true);
    let semantic = encode_module(&module).expect("crashing structural call encodes");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let argument = TerminalStructuralValue {
        opaque_identity: 0xc013,
        structural_type: structural_type_id(1),
        qualifications: vec![structural_domain_id(1)],
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: std::slice::from_ref(&argument),
            ..Default::default()
        },
    )
    .expect("verified crashing structural call starts");
    let mut meter = TerminalFuelMeter::unbounded();

    let crashed = execution
        .resume(&mut meter, &mut AcceptTerminalEffects)
        .unwrap();
    assert!(matches!(
        &crashed,
        TerminalExecutionStatus::Crashed(crash)
            if crash.site == terminal_interpreter::TerminalCrashSite::Edge(edge_id(2))
                && crash.frontier_lower_bound == vec![claim_id(1)]
    ));
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        crashed
    );
    assert_eq!(meter.usage().total_units(), 2);
}

#[test]
fn crashing_multi_claim_structural_callee_preserves_the_exact_abandonment_frontier() {
    let module = multi_claim_internal_structural_call_module(true);
    let semantic = encode_module(&module).expect("crashing multi-claim call encodes");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let argument = TerminalStructuralValue {
        opaque_identity: 0xc015,
        structural_type: structural_type_id(1),
        qualifications: vec![structural_domain_id(1)],
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: std::slice::from_ref(&argument),
            ..Default::default()
        },
    )
    .expect("verified crashing multi-claim call starts");
    let mut meter = TerminalFuelMeter::unbounded();

    let crashed = execution
        .resume(&mut meter, &mut AcceptTerminalEffects)
        .unwrap();
    assert!(matches!(
        &crashed,
        TerminalExecutionStatus::Crashed(crash)
            if crash.site == terminal_interpreter::TerminalCrashSite::Edge(edge_id(2))
                && crash.frontier_lower_bound == vec![claim_id(1), claim_id(2)]
    ));
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        crashed
    );
    assert_eq!(meter.usage().total_units(), 2);
}

#[test]
fn unit_return_performs_affine_discard_only_after_edge_charge() {
    let mut module = effect_module();
    let mut machine = module.machines.pop().expect("callee machine");
    machine.blocks[0].operations.clear();
    machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    machine.entry_claims.clear();
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut machine.blocks[0].terminator
    else {
        unreachable!()
    };
    *trivial_affine_discards = vec![place_id(2)];
    module.entry = machine.id;
    module.machines = vec![machine];
    module.root_service_reach = Default::default();
    let semantic = encode_module(&module).expect("affine cleanup module encodes");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[structural_value(48)],
            ..Default::default()
        },
    )
    .expect("verified affine cleanup should start");
    let mut meter = TerminalFuelMeter::with_allowance(0);

    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(1).unwrap();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
}
