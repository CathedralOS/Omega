use super::*;

#[test]
fn checked_source_booleans_survive_frontend_drop() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi Boolean source canary should compile");
    let constant = lower_machine(&checked, "terminal_boolean_constant")
        .expect("Boolean constant source should lower");
    let parameter = lower_machine(&checked, "terminal_ninth_boolean")
        .expect("Boolean parameter source should lower");
    let chain = lower_machine(&checked, "terminal_boolean_chain")
        .expect("Boolean state chain should lower");
    drop(checked);

    let constant_verified = verify_module(
        &constant.semantic_module,
        &constant.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source Boolean constant should verify");
    let constant_fuel = derive_fixed_entry_fuel(&constant_verified, constant.semantic_module.entry)
        .expect("Boolean constant should have fixed fuel");
    assert_eq!(constant_fuel.ceiling_units(), 2);
    let constant_result = interpret_verified_artifact(&constant_verified, &[])
        .expect("source Boolean constant should execute");
    assert_eq!(
        constant_result.value(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
    );
    assert_eq!(constant_result.usage().total_units(), 2);

    let parameter_verified = verify_module(
        &parameter.semantic_module,
        &parameter.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source Boolean parameter should verify");
    let parameter_fuel =
        derive_fixed_entry_fuel(&parameter_verified, parameter.semantic_module.entry)
            .expect("Boolean parameter should have fixed fuel");
    assert_eq!(parameter_fuel.ceiling_units(), 1);
    let arguments = [false, false, false, false, false, false, false, false, true]
        .into_iter()
        .map(TerminalScalarValue::Boolean)
        .collect::<Vec<_>>();
    let parameter_result = interpret_verified_artifact(&parameter_verified, &arguments)
        .expect("source Boolean parameter should execute");
    assert_eq!(
        parameter_result.value(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
    );
    assert_eq!(parameter_result.usage().total_units(), 1);

    let chain_verified = verify_module(
        &chain.semantic_module,
        &chain.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source Boolean state chain should verify");
    let chain_fuel = derive_fixed_entry_fuel(&chain_verified, chain.semantic_module.entry)
        .expect("Boolean state chain should have fixed fuel");
    assert_eq!(chain_fuel.ceiling_units(), 3);
    let chain_result = interpret_verified_artifact(&chain_verified, &arguments)
        .expect("source Boolean state chain should execute");
    assert_eq!(
        chain_result.value(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
    );
    assert_eq!(chain_result.usage().total_units(), 3);
}

#[cfg(unix)]
#[test]
fn checked_source_boolean_not_round_trips_and_reaches_native_code() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi Boolean-not source canary should compile");
    let lowered =
        lower_machine(&checked, "terminal_boolean_not").expect("Boolean logical not should lower");
    drop(checked);

    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    assert!(matches!(
        lowered.semantic_module.machines[0].blocks[0].operations[0].kind,
        OperationKind::BooleanNot { .. }
    ));
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("Boolean-not terminal Psi should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("Boolean-not terminal Psi should decode");
    let verified = verify_module(
        &semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("Boolean-not terminal Psi should verify");
    let fuel = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("Boolean not should have fixed fuel");
    assert_eq!(fuel.ceiling_units(), 2);
    for (input, expected) in [(false, true), (true, false)] {
        let measured =
            interpret_verified_artifact(&verified, &[TerminalScalarValue::Boolean(input)])
                .expect("Boolean not should interpret");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
        assert_eq!(measured.usage().total_units(), 2);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("Boolean not should cross the source-independent Omega boundary");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("Boolean not should select for the host");
    assert!(matches!(
        target_operations.functions[0].operation,
        TargetOperation::ReturnBooleanNotParameter { .. }
    ));
    let assigned =
        assign_registers(&target_operations).expect("Boolean-not parameter home should assign");
    let machine_code = emit_machine_code(&assigned).expect("Boolean not should emit");
    let object_artifact =
        build_object_artifact(&machine_code).expect("Boolean not should form an object");
    let entry = object_artifact.entry_function();
    assert_eq!(entry.provenance.operations.len(), 1);
    assert_eq!(
        run_host_machine_code_with_bool(entry.bytes(&object_artifact), false),
        1
    );
    assert_eq!(
        run_host_machine_code_with_bool(entry.bytes(&object_artifact), true),
        0
    );
}

#[cfg(unix)]
#[test]
fn checked_source_boolean_equality_round_trips_and_reaches_native_code() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi Boolean-equality source canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_equal_false")
        .expect("Boolean equality should lower");
    drop(checked);

    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    assert!(
        lowered.semantic_module.machines[0].blocks[0]
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::BooleanEqual { .. }))
    );
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("Boolean-equality terminal Psi should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("Boolean-equality terminal Psi should decode");
    let verified = verify_module(
        &semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("Boolean-equality terminal Psi should verify");
    let fuel = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("Boolean equality should have fixed fuel");
    assert_eq!(fuel.ceiling_units(), 3);
    for (input, expected) in [(false, true), (true, false)] {
        let measured =
            interpret_verified_artifact(&verified, &[TerminalScalarValue::Boolean(input)])
                .expect("Boolean equality should interpret");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
        assert_eq!(measured.usage().total_units(), 3);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("Boolean equality should cross the source-independent Omega boundary");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("Boolean equality against false should select for the host");
    assert!(matches!(
        target_operations.functions[0].operation,
        TargetOperation::ReturnBooleanNotParameter { .. }
    ));
    let assigned = assign_registers(&target_operations)
        .expect("Boolean-equality parameter home should assign");
    let machine_code = emit_machine_code(&assigned).expect("Boolean equality should emit");
    let object_artifact =
        build_object_artifact(&machine_code).expect("Boolean equality should form an object");
    let entry = object_artifact.entry_function();
    assert_eq!(entry.provenance.operations.len(), 2);
    assert_eq!(
        run_host_machine_code_with_bool(entry.bytes(&object_artifact), false),
        1
    );
    assert_eq!(
        run_host_machine_code_with_bool(entry.bytes(&object_artifact), true),
        0
    );
}

#[cfg(unix)]
#[test]
fn checked_source_runtime_boolean_equality_reaches_native_code() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("runtime Boolean-equality source canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_equal_runtime")
        .expect("runtime Boolean equality should lower");
    drop(checked);

    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("runtime Boolean equality should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("runtime Boolean equality should decode");
    let verified = verify_module(
        &semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("runtime Boolean equality should verify");
    let fuel = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("runtime Boolean equality should have fixed fuel");
    assert_eq!(fuel.ceiling_units(), 2);
    for (left, right, expected) in [
        (false, false, true),
        (false, true, false),
        (true, false, false),
        (true, true, true),
    ] {
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(left),
                TerminalScalarValue::Boolean(right),
            ],
        )
        .expect("runtime Boolean equality should interpret");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
        assert_eq!(measured.usage().total_units(), 2);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("runtime Boolean equality should cross the Omega boundary");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("runtime Boolean equality should select for the host");
    assert!(matches!(
        &target_operations.functions[0].operation,
        TargetOperation::ReturnBooleanExpression {
            expression: TargetBooleanExpression::Equal { .. },
            ..
        }
    ));
    let assigned = assign_registers(&target_operations)
        .expect("runtime Boolean expression homes should assign");
    let machine_code = emit_machine_code(&assigned).expect("runtime Boolean equality should emit");
    let object_artifact = build_object_artifact(&machine_code)
        .expect("runtime Boolean equality should form an object");
    let entry = object_artifact.entry_function();
    assert_eq!(entry.provenance.operations.len(), 1);
    for (left, right, expected) in [
        (false, false, 1),
        (false, true, 0),
        (true, false, 0),
        (true, true, 1),
    ] {
        assert_eq!(
            run_host_machine_code_with_two_bools(entry.bytes(&object_artifact), left, right,),
            expected
        );
    }
}

#[test]
fn checked_source_runtime_integer_equality_round_trips_and_reaches_native_code() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("runtime integer-equality source canary should compile");
    let lowered = lower_machine(&checked, "terminal_integer_equal_runtime")
        .expect("runtime integer equality should lower");
    drop(checked);

    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    assert!(matches!(
        lowered.semantic_module.machines[0].blocks[0].operations[0].kind,
        OperationKind::IntegerEqual { .. }
    ));
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("runtime integer equality should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("runtime integer equality should decode");
    let verified = verify_module(
        &semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("runtime integer equality should verify");
    let fuel = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("runtime integer equality should have fixed fuel");
    assert_eq!(fuel.ceiling_units(), 2);
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 terminal type");
    for (left, right, expected) in [
        (0_u64, 0_u64, true),
        (0, 1, false),
        (u64::MAX, u64::MAX, true),
        (u64::MAX, u64::MAX - 1, false),
    ] {
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Integer {
                    scalar_type: integer_type,
                    value: IntegerValue::Unsigned(u128::from(left)),
                },
                TerminalScalarValue::Integer {
                    scalar_type: integer_type,
                    value: IntegerValue::Unsigned(u128::from(right)),
                },
            ],
        )
        .expect("runtime integer equality should interpret");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
        assert_eq!(measured.usage().total_units(), 2);
    }

    #[cfg(unix)]
    {
        let abstract_operations = lower_verified_artifact(&verified)
            .expect("runtime integer equality should cross the Omega boundary");
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("runtime integer equality should select for the host");
        assert!(matches!(
            &target_operations.functions[0].operation,
            TargetOperation::ReturnBooleanExpression {
                expression: TargetBooleanExpression::IntegerEqual { .. },
                ..
            }
        ));
        let assigned = assign_registers(&target_operations)
            .expect("runtime integer equality homes should assign");
        let machine_code =
            emit_machine_code(&assigned).expect("runtime integer equality should emit");
        let object_artifact = build_object_artifact(&machine_code)
            .expect("runtime integer equality should form an object");
        let entry = object_artifact.entry_function();
        assert_eq!(entry.provenance.operations.len(), 1);
        for (left, right, expected) in [
            (0_u64, 0_u64, 1),
            (0, 1, 0),
            (u64::MAX, u64::MAX, 1),
            (u64::MAX, u64::MAX - 1, 0),
        ] {
            assert_eq!(
                run_host_machine_code_with_two_u64(entry.bytes(&object_artifact), left, right),
                expected
            );
        }
    }
}

#[test]
fn checked_source_runtime_integer_ordering_round_trips_and_preserves_signedness() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("runtime integer-ordering source canary should compile");
    for (machine, inclusive, scalar_type, cases) in [
        (
            "terminal_unsigned_less_runtime",
            false,
            IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
            vec![
                (IntegerValue::Unsigned(0), IntegerValue::Unsigned(1), true),
                (
                    IntegerValue::Unsigned(u64::MAX.into()),
                    IntegerValue::Unsigned(0),
                    false,
                ),
            ],
        ),
        (
            "terminal_signed_less_or_equal_runtime",
            true,
            IntegerType::new(IntegerSign::Signed, 64).expect("i64"),
            vec![
                (IntegerValue::Signed(-1), IntegerValue::Signed(0), true),
                (IntegerValue::Signed(1), IntegerValue::Signed(0), false),
                (IntegerValue::Signed(1), IntegerValue::Signed(1), true),
            ],
        ),
    ] {
        let lowered = lower_machine(&checked, machine).expect("integer ordering should lower");
        assert_eq!(
            lowered.semantic_module.vocabulary_marker,
            VocabularyMarker::CURRENT
        );
        assert!(
            matches!(
                lowered.semantic_module.machines[0].blocks[0].operations[0].kind,
                OperationKind::IntegerLessOrEqual { .. } if inclusive
            ) || matches!(
                lowered.semantic_module.machines[0].blocks[0].operations[0].kind,
                OperationKind::IntegerLessThan { .. } if !inclusive
            )
        );
        let bytes = encode_module(&lowered.semantic_module).expect("ordering encodes");
        let decoded = decode_module(&bytes).expect("ordering decodes");
        let verified = verify_module(
            &decoded,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .expect("ordering verifies");
        assert_eq!(
            derive_fixed_entry_fuel(&verified, decoded.entry)
                .expect("ordering has fixed fuel")
                .ceiling_units(),
            2
        );
        for (left, right, expected) in &cases {
            let measured = interpret_verified_artifact(
                &verified,
                &[
                    TerminalScalarValue::Integer {
                        scalar_type,
                        value: *left,
                    },
                    TerminalScalarValue::Integer {
                        scalar_type,
                        value: *right,
                    },
                ],
            )
            .expect("ordering interprets");
            assert_eq!(
                measured.value(),
                TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(*expected))
            );
            assert_eq!(measured.usage().total_units(), 2);
        }

        let abstract_operations =
            lower_verified_artifact(&verified).expect("ordering crosses the Omega boundary");
        let portable_target =
            lower_to_target_operations(&abstract_operations, NativeTarget::linux_x64())
                .expect("ordering selects for x86-64");
        let portable_expression = match &portable_target.functions[0].operation {
            TargetOperation::ReturnBooleanExpression { expression, .. } => expression,
            operation => panic!("unexpected ordering operation: {operation:?}"),
        };
        assert!(
            matches!(
                portable_expression,
                TargetBooleanExpression::IntegerLessOrEqual { .. } if inclusive
            ) || matches!(
                portable_expression,
                TargetBooleanExpression::IntegerLessThan { .. } if !inclusive
            )
        );
        let portable_assigned =
            assign_registers(&portable_target).expect("ordering homes assign for x86-64");
        emit_machine_code(&portable_assigned).expect("ordering emits for x86-64");

        #[cfg(unix)]
        {
            let abstract_operations = lower_verified_artifact(&verified).expect("Omega lowering");
            let target_operations =
                lower_to_target_operations(&abstract_operations, NativeTarget::host())
                    .expect("host selection");
            let expected_expression = match &target_operations.functions[0].operation {
                TargetOperation::ReturnBooleanExpression { expression, .. } => expression,
                operation => panic!("unexpected ordering operation: {operation:?}"),
            };
            assert!(
                matches!(
                    expected_expression,
                    TargetBooleanExpression::IntegerLessOrEqual { .. } if inclusive
                ) || matches!(
                    expected_expression,
                    TargetBooleanExpression::IntegerLessThan { .. } if !inclusive
                )
            );
            let assigned = assign_registers(&target_operations).expect("ordering homes assign");
            let machine_code = emit_machine_code(&assigned).expect("ordering emits");
            let object = build_object_artifact(&machine_code).expect("ordering object");
            let entry = object.entry_function();
            for (left, right, expected) in &cases {
                let left = match left {
                    IntegerValue::Unsigned(value) => *value as u64,
                    IntegerValue::Signed(value) => *value as i64 as u64,
                };
                let right = match right {
                    IntegerValue::Unsigned(value) => *value as u64,
                    IntegerValue::Signed(value) => *value as i64 as u64,
                };
                assert_eq!(
                    run_host_machine_code_with_two_u64(entry.bytes(&object), left, right),
                    i32::from(*expected)
                );
            }
        }
    }
}

#[test]
fn checked_source_computed_integer_comparison_reaches_native_code() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("computed integer-comparison source canary should compile");
    let lowered = lower_machine(&checked, "terminal_computed_greater_runtime")
        .expect("computed integer comparison should lower");
    drop(checked);

    assert!(matches!(
        &lowered.semantic_module.machines[0].blocks[0].operations[..],
        [
            psi_terminal::Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            psi_terminal::Operation {
                kind: OperationKind::WrappingIntegerMultiply { .. },
                ..
            },
            psi_terminal::Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            psi_terminal::Operation {
                kind: OperationKind::WrappingIntegerAdd { .. },
                ..
            },
            psi_terminal::Operation {
                kind: OperationKind::IntegerLessThan { .. },
                ..
            },
        ]
    ));
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("computed comparison should encode canonically");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("computed-comparison proof should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("computed comparison should decode");
    let proof_bundle =
        decode_proof_bundle(&proof_bytes).expect("computed-comparison proof should decode");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("computed comparison should verify after frontend drop");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("computed comparison should have fixed fuel")
            .ceiling_units(),
        6
    );

    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: u64_type,
        value: IntegerValue::Unsigned(u128::from(value)),
    };
    for (left, right, expected) in [(10_u64, 3_u64, true), (5, 3, false), (u64::MAX, 0, false)] {
        let measured = interpret_verified_artifact(&verified, &[integer(left), integer(right)])
            .expect("computed comparison should interpret");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
        assert_eq!(measured.usage().total_units(), 6);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("computed comparison should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("computed comparison should select for both native targets");
        assert!(matches!(
            &target_operations.functions[0].operation,
            TargetOperation::ReturnBooleanExpression {
                expression: TargetBooleanExpression::IntegerLessThan { .. },
                ..
            }
        ));
        let assigned =
            assign_registers(&target_operations).expect("computed comparison homes should assign");
        emit_machine_code(&assigned).expect("computed comparison should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("computed comparison should select for the host");
        let assigned =
            assign_registers(&target_operations).expect("host comparison homes should assign");
        let machine_code = emit_machine_code(&assigned).expect("host comparison should emit");
        let object = build_object_artifact(&machine_code).expect("comparison object");
        let entry = object.entry_function();
        for (left, right, expected) in [(10_u64, 3_u64, 1), (5, 3, 0), (u64::MAX, 0, 0)] {
            assert_eq!(
                run_host_machine_code_with_two_u64(entry.bytes(&object), left, right),
                expected
            );
        }
    }
}

#[test]
fn checked_source_runtime_integer_bitwise_operations_cross_the_full_pipeline() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("runtime integer-bitwise source canary should compile");
    let cases = [
        (
            "terminal_unsigned_bitwise_and_runtime",
            0b1100_u64,
            0b1010_u64,
            0b1000_u64,
            0_u8,
        ),
        (
            "terminal_unsigned_bitwise_or_runtime",
            0b1100,
            0b0011,
            0b1111,
            1,
        ),
        (
            "terminal_signed_bitwise_xor_runtime",
            u64::MAX,
            (-128_i64) as u64,
            127,
            2,
        ),
    ];
    for (machine, left, right, expected, kind) in cases {
        let lowered = lower_machine(&checked, machine).expect("integer bitwise should lower");
        assert_eq!(
            lowered.semantic_module.vocabulary_marker,
            VocabularyMarker::CURRENT
        );
        let operation = &lowered.semantic_module.machines[0].blocks[0].operations[0].kind;
        assert!(matches!(
            (kind, operation),
            (0, OperationKind::IntegerBitwiseAnd { .. })
                | (1, OperationKind::IntegerBitwiseOr { .. })
                | (2, OperationKind::IntegerBitwiseXor { .. })
        ));
        let bytes = encode_module(&lowered.semantic_module).expect("bitwise module encodes");
        let decoded = decode_module(&bytes).expect("bitwise module decodes");
        let verified = verify_module(
            &decoded,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .expect("bitwise module verifies");
        assert_eq!(
            derive_fixed_entry_fuel(&verified, decoded.entry)
                .expect("bitwise machine has fixed fuel")
                .ceiling_units(),
            2
        );
        let scalar_type = if kind == 2 {
            IntegerType::new(IntegerSign::Signed, 64).expect("i64")
        } else {
            IntegerType::new(IntegerSign::Unsigned, 64).expect("u64")
        };
        let input = |bits: u64| TerminalScalarValue::Integer {
            scalar_type,
            value: if kind == 2 {
                IntegerValue::Signed(bits as i64 as i128)
            } else {
                IntegerValue::Unsigned(bits.into())
            },
        };
        let measured = interpret_verified_artifact(&verified, &[input(left), input(right)])
            .expect("bitwise operation interprets");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(input(expected))
        );
        assert_eq!(measured.usage().total_units(), 2);

        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let abstract_operations =
                lower_verified_artifact(&verified).expect("bitwise crosses the Omega boundary");
            let target_operations = lower_to_target_operations(&abstract_operations, target)
                .expect("bitwise operation selects on both native architectures");
            let expression = match &target_operations.functions[0].operation {
                TargetOperation::ReturnIntegerExpression { expression, .. } => expression,
                operation => panic!("unexpected bitwise operation: {operation:?}"),
            };
            assert!(matches!(
                (kind, expression),
                (0, TargetIntegerExpression::BitwiseAnd { .. })
                    | (1, TargetIntegerExpression::BitwiseOr { .. })
                    | (2, TargetIntegerExpression::BitwiseXor { .. })
            ));
            let assigned =
                assign_registers(&target_operations).expect("bitwise parameter homes assign");
            emit_machine_code(&assigned).expect("bitwise operation emits exact native code");
        }

        #[cfg(unix)]
        {
            let abstract_operations = lower_verified_artifact(&verified).expect("Omega lowering");
            let target_operations =
                lower_to_target_operations(&abstract_operations, NativeTarget::host())
                    .expect("host selection");
            let assigned = assign_registers(&target_operations).expect("bitwise homes assign");
            let machine_code = emit_machine_code(&assigned).expect("bitwise host emission");
            let object = build_object_artifact(&machine_code).expect("bitwise object");
            assert_eq!(
                run_host_machine_code_with_two_u64(
                    object.entry_function().bytes(&object),
                    left,
                    right,
                ),
                expected as i32
            );
        }
    }
}

#[test]
fn checked_source_runtime_integer_bitwise_not_crosses_canonical_artifacts_and_native_targets() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("runtime integer-bitwise-not source canary should compile");
    let lowered = lower_machine(&checked, "terminal_unsigned_bitwise_not_runtime")
        .expect("integer bitwise-not should lower");
    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    assert!(matches!(
        lowered.semantic_module.machines[0].blocks[0].operations[0].kind,
        OperationKind::IntegerBitwiseNot { .. }
    ));
    let semantic_bytes = encode_module(&lowered.semantic_module).expect("not module encodes");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle).expect("not proof encodes");
    drop(checked);
    drop(lowered);

    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let input = |value| TerminalScalarValue::Integer {
        scalar_type,
        value: IntegerValue::Unsigned(value),
    };
    let expected = input(!0x0f0f_u64 as u128 & u64::MAX as u128);
    let measured = interpret_terminal_artifact_measured(
        &semantic_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &[input(0x0f0f), input(0)],
    )
    .expect("canonical bitwise-not artifact should interpret");
    assert_eq!(measured.value(), TerminalExecutionResult::Scalar(expected));
    assert_eq!(measured.usage().total_units(), 2);

    let abstract_operations =
        lower_artifact_sections(&semantic_bytes, &proof_bytes, &AdmissionProfile::default())
            .expect("canonical bitwise-not artifact should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("bitwise-not should select for both native architectures");
        assert!(matches!(
            &target_operations.functions[0].operation,
            TargetOperation::ReturnIntegerExpression {
                expression: TargetIntegerExpression::BitwiseNot { .. },
                ..
            }
        ));
        let assigned = assign_registers(&target_operations).expect("bitwise-not homes assign");
        emit_machine_code(&assigned).expect("bitwise-not emits native code");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("host bitwise-not selection");
        let assigned = assign_registers(&target_operations).expect("host bitwise-not homes assign");
        let machine_code = emit_machine_code(&assigned).expect("host bitwise-not emission");
        let object = build_object_artifact(&machine_code).expect("bitwise-not object");
        assert_eq!(
            run_host_machine_code_with_two_u64(object.entry_function().bytes(&object), 0x0f0f, 0,),
            0xf0,
        );
    }
}

#[test]
fn checked_source_same_carrier_policy_casts_retag_without_terminal_work() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("explicit arithmetic-policy cast source canary should compile");
    let wrapping = lower_machine(&checked, "terminal_explicit_wrapping_cast_add_runtime")
        .expect("same-carrier wrapping casts should select terminal wrapping addition");
    let erasure = lower_machine(&checked, "terminal_explicit_policy_erasure_runtime")
        .expect("same-carrier policy erasure should lower as an identity");
    assert!(matches!(
        &wrapping.semantic_module.machines[0].blocks[0].operations[..],
        [psi_terminal::Operation {
            kind: OperationKind::WrappingIntegerAdd { .. },
            ..
        }]
    ));
    assert!(
        erasure.semantic_module.machines[0].blocks[0]
            .operations
            .is_empty(),
        "a same-carrier policy erasure must not invent executable terminal work"
    );
    let wrapping_semantic =
        encode_module(&wrapping.semantic_module).expect("wrapping-cast module encodes");
    let wrapping_proof =
        encode_proof_bundle(&wrapping.proof_bundle).expect("wrapping-cast proof encodes");
    let erasure_semantic =
        encode_module(&erasure.semantic_module).expect("policy-erasure module encodes");
    let erasure_proof =
        encode_proof_bundle(&erasure.proof_bundle).expect("policy-erasure proof encodes");
    drop(checked);
    drop(wrapping);
    drop(erasure);

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let u8_value = |value| TerminalScalarValue::Integer {
        scalar_type: u8_type,
        value: IntegerValue::Unsigned(value),
    };
    let measured = interpret_terminal_artifact_measured(
        &wrapping_semantic,
        &wrapping_proof,
        &AdmissionProfile::default(),
        &[u8_value(250), u8_value(10)],
    )
    .expect("canonical wrapping-cast artifact should interpret");
    assert_eq!(
        measured.value(),
        TerminalExecutionResult::Scalar(u8_value(4))
    );
    assert_eq!(measured.usage().total_units(), 2);

    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let u64_value = |value| TerminalScalarValue::Integer {
        scalar_type: u64_type,
        value: IntegerValue::Unsigned(value),
    };
    let measured = interpret_terminal_artifact_measured(
        &erasure_semantic,
        &erasure_proof,
        &AdmissionProfile::default(),
        &[u64_value(73), u64_value(0)],
    )
    .expect("canonical policy-erasure artifact should interpret");
    assert_eq!(
        measured.value(),
        TerminalExecutionResult::Scalar(u64_value(73))
    );
    assert_eq!(measured.usage().total_units(), 1);

    let wrapping_abstract = lower_artifact_sections(
        &wrapping_semantic,
        &wrapping_proof,
        &AdmissionProfile::default(),
    )
    .expect("wrapping-cast artifact should cross the Omega boundary");
    let erasure_abstract = lower_artifact_sections(
        &erasure_semantic,
        &erasure_proof,
        &AdmissionProfile::default(),
    )
    .expect("policy-erasure artifact should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let wrapping_target = lower_to_target_operations(&wrapping_abstract, target)
            .expect("wrapping-cast expression should select on both native targets");
        assert!(matches!(
            &wrapping_target.functions[0].operation,
            TargetOperation::ReturnIntegerExpression {
                expression: TargetIntegerExpression::WrappingAdd { .. },
                ..
            }
        ));
        let wrapping_assigned =
            assign_registers(&wrapping_target).expect("wrapping-cast homes should assign");
        emit_machine_code(&wrapping_assigned).expect("wrapping-cast expression should emit");

        let erasure_target = lower_to_target_operations(&erasure_abstract, target)
            .expect("policy erasure should select on both native targets");
        assert!(matches!(
            erasure_target.functions[0].operation,
            TargetOperation::ReturnIntegerParameter { .. }
        ));
        let erasure_assigned =
            assign_registers(&erasure_target).expect("policy-erasure homes should assign");
        emit_machine_code(&erasure_assigned).expect("policy erasure should emit");
    }

    #[cfg(unix)]
    {
        let wrapping_target = lower_to_target_operations(&wrapping_abstract, NativeTarget::host())
            .expect("host wrapping-cast selection");
        let wrapping_assigned =
            assign_registers(&wrapping_target).expect("host wrapping-cast homes should assign");
        let wrapping_code =
            emit_machine_code(&wrapping_assigned).expect("host wrapping-cast emission");
        let wrapping_object = build_object_artifact(&wrapping_code).expect("wrapping-cast object");
        assert_eq!(
            run_host_machine_code_with_two_u64(
                wrapping_object.entry_function().bytes(&wrapping_object),
                250,
                10,
            ),
            4,
        );

        let erasure_target = lower_to_target_operations(&erasure_abstract, NativeTarget::host())
            .expect("host policy-erasure selection");
        let erasure_assigned =
            assign_registers(&erasure_target).expect("host policy-erasure homes should assign");
        let erasure_code =
            emit_machine_code(&erasure_assigned).expect("host policy-erasure emission");
        let erasure_object = build_object_artifact(&erasure_code).expect("policy-erasure object");
        assert_eq!(
            run_host_machine_code_with_two_u64(
                erasure_object.entry_function().bytes(&erasure_object),
                73,
                0,
            ),
            73,
        );
    }
}

#[test]
fn checked_source_total_integer_widening_crosses_canonical_artifacts_and_native_targets() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("total integer-widening source canaries should compile");
    let cases = [
        (
            "terminal_unsigned_widen_runtime",
            IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
            IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
            IntegerValue::Unsigned(250),
            IntegerValue::Unsigned(250),
            2_u64,
            false,
        ),
        (
            "terminal_signed_widen_runtime",
            IntegerType::new(IntegerSign::Signed, 8).expect("i8"),
            IntegerType::new(IntegerSign::Signed, 64).expect("i64"),
            IntegerValue::Signed(-128),
            IntegerValue::Signed(-128),
            2,
            false,
        ),
        (
            "terminal_unsigned_to_signed_widen_runtime",
            IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
            IntegerType::new(IntegerSign::Signed, 16).expect("i16"),
            IntegerValue::Unsigned(255),
            IntegerValue::Signed(255),
            2,
            false,
        ),
        (
            "terminal_unsigned_widen_then_wrapping_add",
            IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
            IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
            IntegerValue::Unsigned(250),
            IntegerValue::Unsigned(251),
            4,
            true,
        ),
    ];

    for (machine, source_type, target_type, input, expected, fuel, nested_add) in cases {
        let lowered = lower_machine(&checked, machine)
            .unwrap_or_else(|error| panic!("{machine} should lower: {error:?}"));
        assert_eq!(
            lowered.semantic_module.vocabulary_marker,
            VocabularyMarker::CURRENT
        );
        assert!(
            lowered.semantic_module.machines[0]
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| matches!(operation.kind, OperationKind::IntegerWiden { .. })),
            "{machine} must retain widening as terminal work"
        );
        let semantic = encode_module(&lowered.semantic_module)
            .unwrap_or_else(|error| panic!("{machine} semantic module should encode: {error:?}"));
        let proof = encode_proof_bundle(&lowered.proof_bundle)
            .unwrap_or_else(|error| panic!("{machine} proof should encode: {error:?}"));
        drop(lowered);

        let argument = |value| TerminalScalarValue::Integer {
            scalar_type: source_type,
            value,
        };
        let zero = match source_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(0),
            IntegerSign::Unsigned => IntegerValue::Unsigned(0),
        };
        let measured = interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(input), argument(zero)],
        )
        .unwrap_or_else(|error| panic!("{machine} artifact should interpret: {error:?}"));
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                scalar_type: target_type,
                value: expected,
            }),
            "{machine} result"
        );
        assert_eq!(measured.usage().total_units(), fuel, "{machine} fuel");

        let abstract_operations =
            lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
                .unwrap_or_else(|error| panic!("{machine} should cross Omega: {error:?}"));
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let target_operations = lower_to_target_operations(&abstract_operations, target)
                .unwrap_or_else(|error| panic!("{machine} should select: {error:?}"));
            let TargetOperation::ReturnIntegerExpression { expression, .. } =
                &target_operations.functions[0].operation
            else {
                panic!("{machine} should return an integer expression");
            };
            if nested_add {
                let TargetIntegerExpression::WrappingAdd { left, .. } = expression else {
                    panic!("{machine} should retain its wrapping add");
                };
                assert!(matches!(
                    left.as_ref(),
                    TargetIntegerExpression::IntegerWiden { .. }
                ));
            } else {
                assert!(matches!(
                    expression,
                    TargetIntegerExpression::IntegerWiden { .. }
                ));
            }
            let assigned = assign_registers(&target_operations)
                .unwrap_or_else(|error| panic!("{machine} homes should assign: {error:?}"));
            emit_machine_code(&assigned)
                .unwrap_or_else(|error| panic!("{machine} should emit: {error:?}"));
        }

        #[cfg(unix)]
        {
            let target_operations =
                lower_to_target_operations(&abstract_operations, NativeTarget::host())
                    .unwrap_or_else(|error| panic!("{machine} host selection: {error:?}"));
            let assigned = assign_registers(&target_operations)
                .unwrap_or_else(|error| panic!("{machine} host homes: {error:?}"));
            let machine_code = emit_machine_code(&assigned)
                .unwrap_or_else(|error| panic!("{machine} host emission: {error:?}"));
            let object = build_object_artifact(&machine_code)
                .unwrap_or_else(|error| panic!("{machine} host object: {error:?}"));
            let bits = |value: IntegerValue| match value {
                IntegerValue::Unsigned(value) => value as u64,
                IntegerValue::Signed(value) => value as i64 as u64,
            };
            assert!(
                host_machine_code_with_two_u64_matches(
                    object.entry_function().bytes(&object),
                    bits(input),
                    0,
                    bits(expected),
                ),
                "{machine} native result"
            );
        }
    }
}

#[test]
fn checked_source_address_identity_survives_artifacts_and_native_realization() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("address identity source canary should compile");
    let lowered = lower_machine(&checked, "terminal_address_reflexive")
        .expect("address identity should lower to terminal Psi");
    let address = IntegerType::address(64).expect("addr");
    let address_scalar = ScalarType::Integer(address);
    let u64_scalar = ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 64).expect("ordinary u64 carrier"),
    );

    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    assert_eq!(
        lowered.semantic_module.machines[0].parameters[0].scalar_type,
        address_scalar
    );
    assert_eq!(
        lowered.semantic_module.machines[0]
            .result
            .scalar()
            .expect("address-returning source machine has a scalar result")
            .scalar_type,
        ScalarType::Boolean
    );
    assert_ne!(address_scalar, u64_scalar);

    let semantic = encode_module(&lowered.semantic_module).expect("address semantic bytes");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("address proof bytes");
    drop(lowered);
    let decoded = decode_module(&semantic).expect("decode address semantic bytes");
    assert!(matches!(
        decoded.machines[0].parameters[0].scalar_type,
        ScalarType::Integer(integer_type) if integer_type.is_address()
    ));

    let input = IntegerValue::Unsigned(0xfedc_ba98_7654_3210);
    let measured = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[TerminalScalarValue::Integer {
            scalar_type: address,
            value: input,
        }],
    )
    .expect("decoded address artifact should interpret");
    assert_eq!(
        measured.value(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
    );
    assert_eq!(measured.usage().total_units(), 2);

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("address artifact should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("address identity should select");
        let TargetOperation::ReturnBooleanExpression { expression, .. } =
            &target_operations.functions[0].operation
        else {
            panic!("address comparison should return a Boolean expression");
        };
        let TargetBooleanExpression::IntegerEqual { scalar_type, .. } = expression else {
            panic!("address comparison should retain integer equality");
        };
        assert!(scalar_type.is_address());
        let assigned = assign_registers(&target_operations).expect("address homes should assign");
        emit_machine_code(&assigned).expect("address identity should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("address host selection");
        let assigned = assign_registers(&target_operations).expect("address host homes");
        let machine_code = emit_machine_code(&assigned).expect("address host emission");
        let object = build_object_artifact(&machine_code).expect("address host object");
        assert!(host_machine_code_with_two_u64_matches(
            object.entry_function().bytes(&object),
            0xfedc_ba98_7654_3210,
            0,
            1,
        ));
    }
}

#[test]
fn checked_source_policy_retags_and_unary_negation_reuse_terminal_arithmetic() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("policy-retag and unary-negation source canaries should compile");
    let cases = [
        (
            "terminal_explicit_saturating_cast_add_runtime",
            0_u8,
            IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
            IntegerValue::Unsigned(250),
            IntegerValue::Unsigned(10),
            IntegerValue::Unsigned(255),
            2,
        ),
        (
            "terminal_wrapping_negate_runtime",
            1,
            IntegerType::new(IntegerSign::Signed, 8).expect("i8"),
            IntegerValue::Signed(-128),
            IntegerValue::Signed(0),
            IntegerValue::Signed(-128),
            3,
        ),
        (
            "terminal_saturating_negate_runtime",
            2,
            IntegerType::new(IntegerSign::Signed, 8).expect("i8"),
            IntegerValue::Signed(-128),
            IntegerValue::Signed(0),
            IntegerValue::Signed(127),
            3,
        ),
    ];

    for (machine, expected_kind, scalar_type, left, right, expected, expected_fuel) in cases {
        let lowered = lower_machine(&checked, machine)
            .unwrap_or_else(|error| panic!("{machine} should lower: {error:?}"));
        let operation = lowered.semantic_module.machines[0].blocks[0]
            .operations
            .last()
            .expect("policy arithmetic should retain one terminal operation");
        assert!(
            matches!(
                (expected_kind, &operation.kind),
                (0, OperationKind::SaturatingIntegerAdd { .. })
                    | (1, OperationKind::WrappingIntegerSubtract { .. })
                    | (2, OperationKind::SaturatingIntegerSubtract { .. })
            ),
            "{machine} terminal operation kind"
        );
        let semantic = encode_module(&lowered.semantic_module)
            .unwrap_or_else(|error| panic!("{machine} semantic module should encode: {error:?}"));
        let proof = encode_proof_bundle(&lowered.proof_bundle)
            .unwrap_or_else(|error| panic!("{machine} proof should encode: {error:?}"));
        drop(lowered);

        let argument = |value| TerminalScalarValue::Integer { scalar_type, value };
        let measured = interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(left), argument(right)],
        )
        .unwrap_or_else(|error| panic!("{machine} artifact should interpret: {error:?}"));
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(argument(expected)),
            "{machine} result"
        );
        assert_eq!(
            measured.usage().total_units(),
            expected_fuel,
            "{machine} fuel"
        );

        let abstract_operations =
            lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
                .unwrap_or_else(|error| panic!("{machine} should cross Omega: {error:?}"));
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let target_operations = lower_to_target_operations(&abstract_operations, target)
                .unwrap_or_else(|error| panic!("{machine} should select: {error:?}"));
            let TargetOperation::ReturnIntegerExpression { expression, .. } =
                &target_operations.functions[0].operation
            else {
                panic!("{machine} should remain an integer expression return");
            };
            assert!(
                matches!(
                    (machine, expression),
                    (
                        "terminal_explicit_saturating_cast_add_runtime",
                        TargetIntegerExpression::SaturatingAdd { .. }
                    ) | (
                        "terminal_wrapping_negate_runtime",
                        TargetIntegerExpression::WrappingSubtract { .. }
                    ) | (
                        "terminal_saturating_negate_runtime",
                        TargetIntegerExpression::SaturatingSubtract { .. }
                    )
                ),
                "{machine} target expression kind"
            );
            let assigned = assign_registers(&target_operations)
                .unwrap_or_else(|error| panic!("{machine} homes should assign: {error:?}"));
            emit_machine_code(&assigned)
                .unwrap_or_else(|error| panic!("{machine} should emit: {error:?}"));
        }

        #[cfg(unix)]
        {
            let target_operations =
                lower_to_target_operations(&abstract_operations, NativeTarget::host())
                    .unwrap_or_else(|error| panic!("{machine} host selection: {error:?}"));
            let assigned = assign_registers(&target_operations)
                .unwrap_or_else(|error| panic!("{machine} host homes: {error:?}"));
            let machine_code = emit_machine_code(&assigned)
                .unwrap_or_else(|error| panic!("{machine} host emission: {error:?}"));
            let object = build_object_artifact(&machine_code)
                .unwrap_or_else(|error| panic!("{machine} host object: {error:?}"));
            let argument_bits = |value: IntegerValue| match value {
                IntegerValue::Unsigned(value) => value as u64,
                IntegerValue::Signed(value) => value as i64 as u64,
            };
            let expected_bits = argument_bits(expected);
            let actual = run_host_machine_code_with_two_u64(
                object.entry_function().bytes(&object),
                argument_bits(left),
                argument_bits(right),
            ) as u32 as u64;
            let mask = if scalar_type.bits() == 64 {
                u64::MAX
            } else {
                (1_u64 << scalar_type.bits()) - 1
            };
            assert_eq!(
                actual & mask,
                expected_bits & mask,
                "{machine} native result"
            );
        }
    }
}

#[test]
fn checked_source_runtime_wrapping_shifts_cross_the_full_pipeline() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("runtime wrapping-shift source canary should compile");
    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let cases = [
        (
            "terminal_unsigned_wrapping_shift_left_runtime",
            TerminalScalarValue::Integer {
                scalar_type: u64_type,
                value: IntegerValue::Unsigned(1),
            },
            TerminalScalarValue::Integer {
                scalar_type: i64_type,
                value: IntegerValue::Signed(-1),
            },
            TerminalScalarValue::Integer {
                scalar_type: u64_type,
                value: IntegerValue::Unsigned(1_u128 << 63),
            },
            true,
        ),
        (
            "terminal_signed_wrapping_shift_right_runtime",
            TerminalScalarValue::Integer {
                scalar_type: i64_type,
                value: IntegerValue::Signed(-8),
            },
            TerminalScalarValue::Integer {
                scalar_type: u64_type,
                value: IntegerValue::Unsigned(65),
            },
            TerminalScalarValue::Integer {
                scalar_type: i64_type,
                value: IntegerValue::Signed(-4),
            },
            false,
        ),
    ];

    for (machine, value, count, expected, left_shift) in cases {
        let lowered = lower_machine(&checked, machine).expect("wrapping shift should lower");
        assert_eq!(
            lowered.semantic_module.vocabulary_marker,
            VocabularyMarker::CURRENT
        );
        assert!(matches!(
            (
                left_shift,
                &lowered.semantic_module.machines[0].blocks[0].operations[0].kind
            ),
            (true, OperationKind::WrappingIntegerShiftLeft { .. })
                | (false, OperationKind::WrappingIntegerShiftRight { .. })
        ));
        let bytes = encode_module(&lowered.semantic_module).expect("shift module encodes");
        let decoded = decode_module(&bytes).expect("shift module decodes");
        let verified = verify_module(
            &decoded,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .expect("shift module verifies");
        assert_eq!(
            derive_fixed_entry_fuel(&verified, decoded.entry)
                .expect("shift machine has fixed fuel")
                .ceiling_units(),
            2
        );
        let measured = interpret_verified_artifact(&verified, &[value, count])
            .expect("wrapping shift interprets");
        assert_eq!(measured.value(), TerminalExecutionResult::Scalar(expected));
        assert_eq!(measured.usage().total_units(), 2);

        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let abstract_operations =
                lower_verified_artifact(&verified).expect("shift crosses the Omega boundary");
            let target_operations = lower_to_target_operations(&abstract_operations, target)
                .expect("shift selects on both native architectures");
            let expression = match &target_operations.functions[0].operation {
                TargetOperation::ReturnIntegerExpression { expression, .. } => expression,
                operation => panic!("unexpected shift operation: {operation:?}"),
            };
            assert!(matches!(
                (left_shift, expression),
                (true, TargetIntegerExpression::WrappingShiftLeft { .. })
                    | (false, TargetIntegerExpression::WrappingShiftRight { .. })
            ));
            let assigned =
                assign_registers(&target_operations).expect("shift parameter homes assign");
            emit_machine_code(&assigned).expect("shift emits exact native code");
        }

        #[cfg(unix)]
        {
            let abstract_operations = lower_verified_artifact(&verified).expect("Omega lowering");
            let target_operations =
                lower_to_target_operations(&abstract_operations, NativeTarget::host())
                    .expect("host selection");
            let assigned = assign_registers(&target_operations).expect("shift homes assign");
            let machine_code = emit_machine_code(&assigned).expect("shift host emission");
            let object = build_object_artifact(&machine_code).expect("shift object");
            let input_bits = match value {
                TerminalScalarValue::Integer { value, .. } => match value {
                    IntegerValue::Unsigned(value) => value as u64,
                    IntegerValue::Signed(value) => value as i64 as u64,
                },
                TerminalScalarValue::Boolean(_) => unreachable!(),
            };
            let count_bits = match count {
                TerminalScalarValue::Integer { value, .. } => match value {
                    IntegerValue::Unsigned(value) => value as u64,
                    IntegerValue::Signed(value) => value as i64 as u64,
                },
                TerminalScalarValue::Boolean(_) => unreachable!(),
            };
            let expected_bits = match expected {
                TerminalScalarValue::Integer { value, .. } => match value {
                    IntegerValue::Unsigned(value) => value as u64,
                    IntegerValue::Signed(value) => value as i64 as u64,
                },
                TerminalScalarValue::Boolean(_) => unreachable!(),
            };
            assert!(
                host_machine_code_with_two_u64_matches(
                    object.entry_function().bytes(&object),
                    input_bits,
                    count_bits,
                    expected_bits,
                ),
                "emitted wrapping shift should return the complete expected u64"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn checked_source_runtime_boolean_inequality_reuses_terminal_primitives() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("runtime Boolean-inequality source canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_not_equal_runtime")
        .expect("runtime Boolean inequality should lower");
    drop(checked);

    let operations = &lowered.semantic_module.machines[0].blocks[0].operations;
    assert_eq!(operations.len(), 2);
    assert!(matches!(
        operations[0].kind,
        OperationKind::BooleanEqual { .. }
    ));
    assert!(matches!(
        operations[1].kind,
        OperationKind::BooleanNot { .. }
    ));
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("runtime Boolean inequality should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("runtime Boolean inequality should decode");
    let verified = verify_module(
        &semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("runtime Boolean inequality should verify");
    let fuel = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("runtime Boolean inequality should have fixed fuel");
    assert_eq!(fuel.ceiling_units(), 3);
    for (left, right, expected) in [
        (false, false, false),
        (false, true, true),
        (true, false, true),
        (true, true, false),
    ] {
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(left),
                TerminalScalarValue::Boolean(right),
            ],
        )
        .expect("runtime Boolean inequality should interpret");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
        assert_eq!(measured.usage().total_units(), 3);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("runtime Boolean inequality should cross the Omega boundary");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("runtime Boolean inequality should select for the host");
    assert!(matches!(
        &target_operations.functions[0].operation,
        TargetOperation::ReturnBooleanExpression {
            expression: TargetBooleanExpression::Not { operand, .. },
            ..
        } if matches!(operand.as_ref(), TargetBooleanExpression::Equal { .. })
    ));
    let assigned = assign_registers(&target_operations)
        .expect("runtime Boolean inequality homes should assign");
    let machine_code =
        emit_machine_code(&assigned).expect("runtime Boolean inequality should emit");
    let object_artifact = build_object_artifact(&machine_code)
        .expect("runtime Boolean inequality should form an object");
    let entry = object_artifact.entry_function();
    assert_eq!(entry.provenance.operations.len(), 2);
    for (left, right, expected) in [
        (false, false, 0),
        (false, true, 1),
        (true, false, 1),
        (true, true, 0),
    ] {
        assert_eq!(
            run_host_machine_code_with_two_bools(entry.bytes(&object_artifact), left, right),
            expected
        );
    }
}

#[cfg(unix)]
#[test]
fn checked_source_short_circuit_booleans_lower_to_terminal_control() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("short-circuit Boolean source canaries should compile");
    for (machine, is_and) in [
        ("terminal_boolean_and", true),
        ("terminal_boolean_or", false),
    ] {
        let lowered = lower_machine(&checked, machine)
            .expect("short-circuit Boolean expression should lower");
        let conditional_count = lowered.semantic_module.machines[0]
            .blocks
            .iter()
            .filter(|block| matches!(block.terminator, Terminator::Conditional { .. }))
            .count();
        assert_eq!(conditional_count, 2);
        let semantic_bytes = encode_module(&lowered.semantic_module)
            .expect("short-circuit Boolean control should encode canonically");
        let semantic_module =
            decode_module(&semantic_bytes).expect("short-circuit Boolean control should decode");
        let verified = verify_module(
            &semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .expect("short-circuit Boolean control should verify");
        let fuel = derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("short-circuit Boolean control should have fixed fuel");
        assert_eq!(fuel.ceiling_units(), 4);

        for (left, right) in [(false, false), (false, true), (true, false), (true, true)] {
            let expected = if is_and { left && right } else { left || right };
            let expected_units = if (is_and && !left) || (!is_and && left) {
                3
            } else {
                4
            };
            let measured = interpret_verified_artifact(
                &verified,
                &[
                    TerminalScalarValue::Boolean(left),
                    TerminalScalarValue::Boolean(right),
                ],
            )
            .expect("short-circuit Boolean control should interpret");
            assert_eq!(
                measured.value(),
                TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
            );
            assert_eq!(measured.usage().total_units(), expected_units);
        }

        let abstract_operations = lower_verified_artifact(&verified)
            .expect("short-circuit Boolean control should cross the Omega boundary");
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("short-circuit Boolean control should select for the host");
        assert!(matches!(
            target_operations.functions[0].operation,
            TargetOperation::ReturnBooleanConditionalControl { .. }
        ));
        let assigned = assign_registers(&target_operations)
            .expect("short-circuit Boolean control homes should assign");
        let machine_code =
            emit_machine_code(&assigned).expect("short-circuit Boolean control should emit");
        let object_artifact = build_object_artifact(&machine_code)
            .expect("short-circuit Boolean control should form an object");
        let entry = object_artifact.entry_function();
        for (left, right) in [(false, false), (false, true), (true, false), (true, true)] {
            let expected = i32::from(if is_and { left && right } else { left || right });
            assert_eq!(
                run_host_machine_code_with_two_bools(entry.bytes(&object_artifact), left, right),
                expected
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn checked_source_short_circuit_expression_conditions_reach_native_control() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("short-circuit expression-condition canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_equal_and_equal")
        .expect("short-circuit expression conditions should lower");
    drop(checked);
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("expression-condition control should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("expression-condition control should decode");
    let verified = verify_module(
        &semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("expression-condition control should verify");
    let fuel = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("expression-condition control should have fixed fuel");
    assert_eq!(fuel.ceiling_units(), 6);

    for (first, second, third) in [
        (false, false, false),
        (false, false, true),
        (false, true, false),
        (false, true, true),
        (true, false, false),
        (true, false, true),
        (true, true, false),
        (true, true, true),
    ] {
        let expected = first == second && second == third;
        let expected_units = if first == second { 6 } else { 4 };
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
                TerminalScalarValue::Boolean(third),
            ],
        )
        .expect("expression-condition control should interpret");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
        assert_eq!(measured.usage().total_units(), expected_units);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("expression-condition control should cross the Omega boundary");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("expression-condition control should select for the host");
    assert!(matches!(
        target_operations.functions[0].operation,
        TargetOperation::ReturnBooleanExpressionConditionalControl { .. }
    ));
    let assigned = assign_registers(&target_operations)
        .expect("expression-condition control homes should assign");
    let machine_code =
        emit_machine_code(&assigned).expect("expression-condition control should emit");
    let object_artifact = build_object_artifact(&machine_code)
        .expect("expression-condition control should form an object");
    let entry = object_artifact.entry_function();
    for (first, second, third) in [
        (false, false, false),
        (false, false, true),
        (false, true, false),
        (false, true, true),
        (true, false, false),
        (true, false, true),
        (true, true, false),
        (true, true, true),
    ] {
        let expected = i32::from(first == second && second == third);
        assert_eq!(
            run_host_machine_code_with_three_bools(
                entry.bytes(&object_artifact),
                first,
                second,
                third,
            ),
            expected
        );
    }
}

#[cfg(unix)]
#[test]
fn checked_source_short_circuit_operands_preserve_terminal_equality() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("short-circuit equality canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_short_circuit_equality")
        .expect("short-circuit equality operands should lower");
    drop(checked);
    assert!(
        lowered.semantic_module.machines[0]
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(operation.kind, OperationKind::BooleanEqual { .. })),
        "value-producing decision leaves must retain the equality operation"
    );

    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("short-circuit equality control should encode");
    let semantic_module =
        decode_module(&semantic_bytes).expect("short-circuit equality control should decode");
    let verified = verify_module(
        &semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("short-circuit equality control should verify");
    let fixed = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("short-circuit equality control should have exact fuel");
    assert_eq!(fixed.ceiling_units(), 8);

    for (first, second, third) in [
        (false, false, false),
        (false, false, true),
        (false, true, false),
        (false, true, true),
        (true, false, false),
        (true, false, true),
        (true, true, false),
        (true, true, true),
    ] {
        let expected = (first && second) == (second || third);
        let expected_units = 4 + if first { 2 } else { 1 } + if second { 1 } else { 2 };
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
                TerminalScalarValue::Boolean(third),
            ],
        )
        .expect("short-circuit equality control should interpret");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
        assert_eq!(measured.usage().total_units(), expected_units);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("short-circuit equality control should cross the Omega boundary");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("short-circuit equality control should select for the host");
    assert!(matches!(
        target_operations.functions[0].operation,
        TargetOperation::ReturnBooleanConditionalControl { .. }
    ));
    let assigned = assign_registers(&target_operations)
        .expect("short-circuit equality control homes should assign");
    let machine_code =
        emit_machine_code(&assigned).expect("short-circuit equality control should emit");
    let object_artifact = build_object_artifact(&machine_code)
        .expect("short-circuit equality control should form an object");
    let entry = object_artifact.entry_function();
    for (first, second, third) in [
        (false, false, false),
        (false, false, true),
        (false, true, false),
        (false, true, true),
        (true, false, false),
        (true, false, true),
        (true, true, false),
        (true, true, true),
    ] {
        assert_eq!(
            run_host_machine_code_with_three_bools(
                entry.bytes(&object_artifact),
                first,
                second,
                third,
            ),
            i32::from((first && second) == (second || third))
        );
    }
}

#[cfg(unix)]
#[test]
fn source_booleans_reach_constant_and_stack_parameter_machine_code() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi Boolean source canary should compile");
    let lowered = [
        ("terminal_boolean_constant", true),
        ("terminal_ninth_boolean", false),
    ]
    .into_iter()
    .map(|(machine, has_operation)| {
        (
            machine,
            has_operation,
            lower_machine(&checked, machine)
                .unwrap_or_else(|error| panic!("{machine} should lower: {error:?}")),
        )
    })
    .collect::<Vec<_>>();
    drop(checked);

    for (machine, has_operation, lowered) in lowered {
        let verified = verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .unwrap_or_else(|error| panic!("{machine} should verify: {error:?}"));
        let abstract_operations = lower_verified_artifact(&verified)
            .unwrap_or_else(|error| panic!("{machine} should lower: {error:?}"));
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .unwrap_or_else(|error| panic!("{machine} should select: {error:?}"));
        let assigned =
            assign_registers(&target_operations).expect("Boolean target homes should assign");
        let machine_code = emit_machine_code(&assigned)
            .unwrap_or_else(|error| panic!("{machine} should emit: {error:?}"));
        let object_artifact = build_object_artifact(&machine_code)
            .unwrap_or_else(|error| panic!("{machine} should form an object: {error:?}"));
        let entry = object_artifact.entry_function();
        assert_eq!(!entry.provenance.operations.is_empty(), has_operation);
        let exit = if has_operation {
            run_host_machine_code(entry.bytes(&object_artifact))
        } else {
            run_host_machine_code_with_nine_bool(entry.bytes(&object_artifact))
        };
        assert_eq!(exit, 1, "{machine} native Boolean result");
    }
}

#[cfg(unix)]
#[test]
fn source_boolean_jump_bindings_reach_stack_parameter_machine_code() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi Boolean state-chain canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_chain")
        .expect("Boolean state chain should lower");
    drop(checked);

    let verified = verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("Boolean state chain should verify");
    let abstract_operations = lower_verified_artifact(&verified)
        .expect("Boolean jump bindings should lower without frontend state");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("Boolean jump bindings should select for the host");
    let assigned =
        assign_registers(&target_operations).expect("Boolean jump target homes should assign");
    let machine_code = emit_machine_code(&assigned).expect("Boolean jump bindings should emit");
    let object_artifact =
        build_object_artifact(&machine_code).expect("Boolean state chain should form an object");
    let entry = object_artifact.entry_function();
    assert!(entry.provenance.operations.is_empty());
    assert_eq!(
        entry.provenance.edges,
        [
            EdgeId::new(1).expect("first jump edge"),
            EdgeId::new(2).expect("second jump edge"),
            EdgeId::new(3).expect("return edge"),
        ]
    );
    assert_eq!(
        run_host_machine_code_with_nine_bool(entry.bytes(&object_artifact)),
        1
    );
}

#[cfg(unix)]
#[test]
fn source_boolean_state_chain_return_preserves_short_circuit_control() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi Boolean state-chain canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_chain_short_circuit_return")
        .expect("Boolean state-chain short-circuit return should lower");
    drop(checked);

    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("state-chain short-circuit control should encode");
    let semantic_module =
        decode_module(&semantic_bytes).expect("state-chain short-circuit control should decode");
    let verified = verify_module(
        &semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("state-chain short-circuit control should verify");
    let fuel = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("state-chain short-circuit control should have fixed fuel");
    assert_eq!(fuel.ceiling_units(), 6);

    for (value, expected_units) in [(false, 6), (true, 4)] {
        let measured =
            interpret_verified_artifact(&verified, &[TerminalScalarValue::Boolean(value)])
                .expect("state-chain short-circuit control should interpret");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
        );
        assert_eq!(measured.usage().total_units(), expected_units);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("state-chain short-circuit control should cross the Omega boundary");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("state-chain short-circuit control should select for the host");
    assert!(matches!(
        target_operations.functions[0].operation,
        TargetOperation::ReturnBooleanConditionalControl { .. }
    ));
    let assigned = assign_registers(&target_operations)
        .expect("state-chain short-circuit control homes should assign");
    let machine_code =
        emit_machine_code(&assigned).expect("state-chain short-circuit control should emit");
    let object_artifact = build_object_artifact(&machine_code)
        .expect("state-chain short-circuit control should form an object");
    let entry = object_artifact.entry_function();
    assert_eq!(
        run_host_machine_code_with_bool(entry.bytes(&object_artifact), false),
        1
    );
    assert_eq!(
        run_host_machine_code_with_bool(entry.bytes(&object_artifact), true),
        1
    );
}

#[cfg(unix)]
#[test]
fn source_boolean_state_chain_binding_preserves_short_circuit_control() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi Boolean state-chain canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_chain_short_circuit_binding")
        .expect("Boolean state-chain short-circuit binding should lower");
    drop(checked);

    let semantic_bytes =
        encode_module(&lowered.semantic_module).expect("state-chain binding control should encode");
    let semantic_module =
        decode_module(&semantic_bytes).expect("state-chain binding control should decode");
    let verified = verify_module(
        &semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("state-chain binding control should verify");
    let fuel = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("state-chain binding control should have fixed fuel");
    assert_eq!(fuel.ceiling_units(), 6);

    for (first, second) in [(false, false), (false, true), (true, false), (true, true)] {
        let expected = first && second;
        let expected_units = if first { 6 } else { 5 };
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
            ],
        )
        .expect("state-chain binding control should interpret");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
        assert_eq!(measured.usage().total_units(), expected_units);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("state-chain binding control should cross the Omega boundary");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("state-chain binding control should select for the host");
    assert!(matches!(
        target_operations.functions[0].operation,
        TargetOperation::ReturnBooleanConditionalControl { .. }
    ));
    let assigned = assign_registers(&target_operations)
        .expect("state-chain binding control homes should assign");
    let machine_code =
        emit_machine_code(&assigned).expect("state-chain binding control should emit");
    let object_artifact = build_object_artifact(&machine_code)
        .expect("state-chain binding control should form an object");
    let entry = object_artifact.entry_function();
    for (first, second) in [(false, false), (false, true), (true, false), (true, true)] {
        assert_eq!(
            run_host_machine_code_with_two_bools(entry.bytes(&object_artifact), first, second,),
            i32::from(first && second)
        );
    }
}
