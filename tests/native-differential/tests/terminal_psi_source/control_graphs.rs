use super::*;

#[test]
fn checked_source_conditional_survives_frontend_drop() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi conditional source canary should compile");
    let lowered = lower_machine(&checked, "terminal_runtime_conditional")
        .expect("ordered source conditional should lower");
    drop(checked);

    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("source conditional should encode canonically");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("source conditional proof should encode canonically");
    drop(lowered);
    let semantic_module = decode_module(&semantic_bytes).expect("decode source conditional");
    let proof_bundle = decode_proof_bundle(&proof_bytes).expect("decode source conditional proof");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source conditional should verify after frontend drop");
    assert_eq!(semantic_module.vocabulary_marker, VocabularyMarker::CURRENT);
    let fixed = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("source conditional should have an exact maximum fuel bound");
    assert_eq!(fixed.ceiling_units(), 5);

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    for (condition, expected, selected, unselected) in [
        (
            true,
            49_u128,
            EdgeId::new(1).unwrap(),
            EdgeId::new(2).unwrap(),
        ),
        (false, 239, EdgeId::new(2).unwrap(), EdgeId::new(1).unwrap()),
    ] {
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(condition),
                TerminalScalarValue::Integer {
                    scalar_type: u8_type,
                    value: IntegerValue::Unsigned(17),
                },
                TerminalScalarValue::Integer {
                    scalar_type: u8_type,
                    value: IntegerValue::Unsigned(29),
                },
            ],
        )
        .expect("selected source conditional arm should execute");
        assert_eq!(measured.usage().total_units(), 5);
        assert!(
            measured
                .usage()
                .at(FuelChargeSite::Edge(selected))
                .is_some()
        );
        assert_eq!(measured.usage().at(FuelChargeSite::Edge(unselected)), None);
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                scalar_type: u8_type,
                value: IntegerValue::Unsigned(expected),
            })
        );
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("source conditional should cross the Omega abstract boundary");
    let AbstractOperation::Conditional {
        when_true,
        when_false,
        ..
    } = &abstract_operations.functions[0].operations[0]
    else {
        panic!("abstract plan must retain the conditional")
    };
    assert_eq!(when_true.bindings.len(), 2);
    assert_eq!(when_false.bindings.len(), 2);
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("computed source conditional should lower for the host");
    let TargetOperation::ReturnIntegerConditionalControl {
        when_true,
        when_false,
        ..
    } = &target_operations.functions[0].operation
    else {
        panic!("target plan must retain both computed conditional expressions")
    };
    let TargetIntegerControl::Return {
        expression: true_expression,
        ..
    } = when_true.control.as_ref()
    else {
        panic!("true source arm must return")
    };
    assert!(matches!(
        true_expression,
        TargetIntegerExpression::WrappingAdd { .. }
    ));
    let TargetIntegerControl::Return {
        expression: false_expression,
        ..
    } = when_false.control.as_ref()
    else {
        panic!("false source arm must return")
    };
    assert!(matches!(
        false_expression,
        TargetIntegerExpression::WrappingAdd { left, .. }
            if matches!(
                left.as_ref(),
                TargetIntegerExpression::WrappingMultiply { .. }
            )
    ));
    assert_eq!(
        target_operations.functions[0].provenance.operations.len(),
        6
    );
    let assigned = assign_registers(&target_operations)
        .expect("source conditional parameter homes should assign");
    let _machine_code =
        emit_machine_code(&assigned).expect("source conditional machine code should emit");
    #[cfg(unix)]
    for (condition, expected) in [(true, 49), (false, 239)] {
        assert_eq!(
            run_host_machine_code_with_conditional_u8(
                &machine_code.functions[0].bytes,
                condition,
                17,
                29,
            ),
            expected
        );
    }
}

#[test]
fn checked_source_acyclic_branch_graph_reaches_both_native_backends() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("nested terminal-Psi branch source canary should compile");
    let lowered = lower_machine(&checked, "terminal_nested_integer_branch")
        .expect("nested ordered source branches should lower");
    drop(checked);

    assert_eq!(lowered.semantic_module.machines[0].blocks.len(), 6);
    assert_eq!(
        lowered.semantic_module.machines[0]
            .blocks
            .iter()
            .filter(|block| matches!(block.terminator, Terminator::Conditional { .. }))
            .count(),
        2
    );
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("nested source branch tree should encode canonically");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("nested source branch proof should encode canonically");
    let semantic_module = decode_module(&semantic_bytes).expect("decode nested source branch tree");
    let proof_bundle =
        decode_proof_bundle(&proof_bytes).expect("decode nested source branch proof");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("nested source branch tree should verify after frontend drop");
    let fixed = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("nested branch tree should have an exact maximum fuel bound");
    assert_eq!(fixed.ceiling_units(), 6);

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: u8_type,
        value: IntegerValue::Unsigned(value),
    };
    for (first, second, expected, units) in [
        (true, true, 11_u128, 6_u64),
        (true, false, 21, 6),
        (false, false, 31, 5),
    ] {
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
                integer(10),
                integer(20),
                integer(30),
            ],
        )
        .expect("nested branch selection should interpret");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(integer(expected))
        );
        assert_eq!(measured.usage().total_units(), units);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("nested branch tree should cross the Omega abstract boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("nested branch tree should select for both native targets");
        assert!(matches!(
            target_operations.functions[0].operation,
            TargetOperation::ReturnIntegerConditionalControl { .. }
        ));
        let assigned = assign_registers(&target_operations)
            .expect("nested branch parameter homes should assign");
        let machine_code =
            emit_machine_code(&assigned).expect("nested branch machine code should emit");
        assert!(!machine_code.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_integer_graph_computes_boolean_jump_bindings() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("computed Boolean integer-graph source canary should compile");
    let lowered = lower_machine(&checked, "terminal_integer_computed_boolean_binding")
        .expect("integer graphs should lower non-short-circuit Boolean bindings");
    drop(checked);

    let machine = &lowered.semantic_module.machines[0];
    assert_eq!(machine.blocks.len(), 3);
    assert!(matches!(
        &machine.blocks[0].operations[..],
        [
            psi_terminal::Operation {
                kind: OperationKind::BooleanEqual { .. },
                ..
            },
            psi_terminal::Operation {
                kind: OperationKind::BooleanNot { .. },
                ..
            },
        ]
    ));
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("computed Boolean integer graph should encode canonically");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("computed Boolean integer-graph proof should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("computed Boolean integer graph should decode");
    let proof_bundle = decode_proof_bundle(&proof_bytes)
        .expect("computed Boolean integer-graph proof should decode");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("computed Boolean integer graph should verify after frontend drop");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("computed Boolean integer graph should have exact fuel")
            .ceiling_units(),
        5
    );

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: u8_type,
        value: IntegerValue::Unsigned(value),
    };
    for (first, second, expected) in [
        (false, false, 20_u128),
        (false, true, 10),
        (true, false, 10),
        (true, true, 20),
    ] {
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
                integer(10),
                integer(20),
            ],
        )
        .expect("computed Boolean integer graph should interpret");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(integer(expected))
        );
        assert_eq!(measured.usage().total_units(), 5);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("computed Boolean integer graph should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("computed Boolean integer graph should select for both native targets");
        assert!(matches!(
            target_operations.functions[0].operation,
            TargetOperation::ReturnIntegerExpressionConditionalControl { .. }
        ));
        let assigned = assign_registers(&target_operations)
            .expect("computed Boolean integer-graph homes should assign");
        let machine_code = emit_machine_code(&assigned)
            .expect("computed Boolean integer-graph machine code should emit");
        assert!(!machine_code.functions[0].bytes.is_empty());
        assert!(
            machine_code.functions[0].scalar_stack.is_some(),
            "linear expression condition and direct integer-return arms should retain scalar stack evidence"
        );
        let artifact = build_object_artifact(&machine_code)
            .expect("computed Boolean integer-graph scalar evidence should validate");
        derive_stack_demand(&artifact, machine_code.entry)
            .expect("computed Boolean integer-graph stack demand should compose");
    }
}

#[test]
fn checked_source_integer_graph_stages_short_circuit_boolean_jump_bindings() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("short-circuit Boolean integer-graph source canary should compile");
    let lowered = lower_machine(&checked, "terminal_integer_short_circuit_boolean_binding")
        .expect("integer graphs should stage short-circuit Boolean bindings");
    drop(checked);

    let machine = &lowered.semantic_module.machines[0];
    assert_eq!(machine.blocks.len(), 11);
    assert_eq!(
        machine
            .blocks
            .iter()
            .filter(|block| matches!(block.terminator, Terminator::Conditional { .. }))
            .count(),
        3
    );
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("short-circuit Boolean integer graph should encode canonically");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("short-circuit Boolean integer-graph proof should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("short-circuit Boolean integer graph should decode");
    let proof_bundle = decode_proof_bundle(&proof_bytes)
        .expect("short-circuit Boolean integer-graph proof should decode");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("short-circuit Boolean integer graph should verify after frontend drop");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("short-circuit Boolean integer graph should have exact fuel")
            .ceiling_units(),
        10
    );

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: u8_type,
        value: IntegerValue::Unsigned(value),
    };
    for (first, second, expected, units) in [
        (false, false, 20_u128, 9_u64),
        (false, true, 20, 9),
        (true, false, 20, 10),
        (true, true, 10, 10),
    ] {
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
                integer(10),
                integer(20),
            ],
        )
        .expect("short-circuit Boolean integer graph should interpret");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(integer(expected))
        );
        assert_eq!(measured.usage().total_units(), units);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("short-circuit Boolean integer graph should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("short-circuit Boolean integer graph should select for both native targets");
        assert!(matches!(
            target_operations.functions[0].operation,
            TargetOperation::ReturnIntegerConditionalControl { .. }
        ));
        let assigned = assign_registers(&target_operations)
            .expect("short-circuit Boolean integer-graph homes should assign");
        let machine_code = emit_machine_code(&assigned)
            .expect("short-circuit Boolean integer-graph machine code should emit");
        assert!(!machine_code.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_integer_graph_localizes_short_circuit_boolean_edge_bindings() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("selected short-circuit Boolean source canary should compile");
    let lowered = lower_machine(
        &checked,
        "terminal_integer_conditional_short_circuit_boolean_binding",
    )
    .expect("integer graphs should localize short-circuit Boolean edge bindings");
    drop(checked);

    let machine = &lowered.semantic_module.machines[0];
    assert_eq!(machine.blocks.len(), 12);
    assert_eq!(
        machine
            .blocks
            .iter()
            .filter(|block| matches!(block.terminator, Terminator::Conditional { .. }))
            .count(),
        4
    );
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("selected short-circuit Boolean graph should encode canonically");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("selected short-circuit Boolean graph proof should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("selected short-circuit Boolean graph should decode");
    let proof_bundle = decode_proof_bundle(&proof_bytes)
        .expect("selected short-circuit Boolean graph proof should decode");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("selected short-circuit Boolean graph should verify after frontend drop");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("selected short-circuit Boolean graph should have exact fuel")
            .ceiling_units(),
        10
    );

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: u8_type,
        value: IntegerValue::Unsigned(value),
    };
    for (select, first, second, expected, units) in [
        (false, true, true, 20_u128, 3_u64),
        (true, false, true, 20, 9),
        (true, true, false, 20, 10),
        (true, true, true, 10, 10),
    ] {
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(select),
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
                integer(10),
                integer(20),
            ],
        )
        .expect("selected short-circuit Boolean graph should interpret");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(integer(expected))
        );
        assert_eq!(measured.usage().total_units(), units);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("selected short-circuit Boolean graph should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("selected short-circuit Boolean graph should select for both native targets");
        assert!(matches!(
            target_operations.functions[0].operation,
            TargetOperation::ReturnIntegerConditionalControl { .. }
        ));
        let assigned = assign_registers(&target_operations)
            .expect("selected short-circuit Boolean graph homes should assign");
        let machine_code = emit_machine_code(&assigned)
            .expect("selected short-circuit Boolean graph machine code should emit");
        assert!(!machine_code.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_unconditional_mixed_scalar_graph_uses_general_lowering() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("unconditional mixed-scalar source canary should compile");
    let lowered = lower_machine(&checked, "terminal_unconditional_mixed_scalar_graph")
        .expect("unconditional mixed-scalar graphs should use general lowering");
    drop(checked);

    let machine = &lowered.semantic_module.machines[0];
    assert_eq!(machine.blocks.len(), 10);
    assert_eq!(
        machine
            .blocks
            .iter()
            .filter(|block| matches!(block.terminator, Terminator::Conditional { .. }))
            .count(),
        2
    );
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("unconditional mixed-scalar graph should encode canonically");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("unconditional mixed-scalar graph proof should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("unconditional mixed-scalar graph should decode");
    let proof_bundle = decode_proof_bundle(&proof_bytes)
        .expect("unconditional mixed-scalar graph proof should decode");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("unconditional mixed-scalar graph should verify after frontend drop");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("unconditional mixed-scalar graph should have exact fuel")
            .ceiling_units(),
        11
    );

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: u8_type,
        value: IntegerValue::Unsigned(value),
    };
    for (first, second, units) in [
        (false, false, 10_u64),
        (false, true, 10),
        (true, false, 11),
        (true, true, 11),
    ] {
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
                integer(30),
            ],
        )
        .expect("unconditional mixed-scalar graph should interpret");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(integer(31))
        );
        assert_eq!(measured.usage().total_units(), units);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("unconditional mixed-scalar graph should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("unconditional mixed-scalar graph should select for both native targets");
        assert!(matches!(
            target_operations.functions[0].operation,
            TargetOperation::ReturnIntegerConditionalControl { .. }
        ));
        let assigned = assign_registers(&target_operations)
            .expect("unconditional mixed-scalar graph homes should assign");
        let machine_code = emit_machine_code(&assigned)
            .expect("unconditional mixed-scalar graph machine code should emit");
        assert!(!machine_code.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_nested_jump_expressions_reach_terminal_and_native_lowering() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("computed nested-jump source canary should compile");
    let lowered = lower_machine(&checked, "terminal_nested_jump_expression")
        .expect("an unconditional nested jump may compute its arguments");
    drop(checked);

    assert_eq!(lowered.semantic_module.machines[0].blocks.len(), 4);
    assert_eq!(
        lowered.semantic_module.machines[0].blocks[1]
            .operations
            .len(),
        2
    );
    assert_eq!(
        lowered.semantic_module.machines[0].blocks[2]
            .operations
            .len(),
        2
    );
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("computed nested jump should encode canonically");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("computed nested-jump proof should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("decode computed nested-jump module");
    let proof_bundle =
        decode_proof_bundle(&proof_bytes).expect("decode computed nested-jump proof");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("computed nested jump should verify after frontend drop");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("computed nested jump should have an exact fuel bound")
            .ceiling_units(),
        5
    );

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: u8_type,
        value: IntegerValue::Unsigned(value),
    };
    for (choose_add, expected) in [(true, 8_u128), (false, 14)] {
        let measured = interpret_verified_artifact(
            &verified,
            &[TerminalScalarValue::Boolean(choose_add), integer(7)],
        )
        .expect("computed nested jump should interpret");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(integer(expected))
        );
        assert_eq!(measured.usage().total_units(), 5);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("computed nested jump should cross the Omega abstract boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("computed nested jump should select for both native targets");
        let assigned = assign_registers(&target_operations)
            .expect("computed nested-jump parameter homes should assign");
        let machine_code =
            emit_machine_code(&assigned).expect("computed nested-jump machine code should emit");
        assert!(!machine_code.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_conditional_edge_expressions_execute_only_on_the_selected_arm() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("computed conditional-edge source canary should compile");
    let lowered = lower_machine(&checked, "terminal_conditional_edge_expression")
        .expect("conditional edges may compute bindings in selected-arm blocks");
    drop(checked);

    let machine = &lowered.semantic_module.machines[0];
    assert_eq!(machine.blocks.len(), 4);
    assert!(matches!(
        machine.blocks[0].terminator,
        Terminator::Conditional {
            ref when_true,
            ref when_false,
            ..
        } if when_true.target.get() == 3 && when_false.target.get() == 4
    ));
    assert!(matches!(
        &machine.blocks[2].operations[..],
        [
            psi_terminal::Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            psi_terminal::Operation {
                kind: OperationKind::WrappingIntegerAdd { .. },
                ..
            },
        ]
    ));
    assert!(matches!(
        &machine.blocks[3].operations[..],
        [
            psi_terminal::Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            psi_terminal::Operation {
                kind: OperationKind::WrappingIntegerMultiply { .. },
                ..
            },
        ]
    ));

    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("computed conditional edge should encode canonically");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("computed conditional-edge proof should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("decode computed conditional-edge module");
    let proof_bundle =
        decode_proof_bundle(&proof_bytes).expect("decode computed conditional-edge proof");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("computed conditional edge should verify after frontend drop");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("computed conditional edge should have an exact fuel bound")
            .ceiling_units(),
        5
    );

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: u8_type,
        value: IntegerValue::Unsigned(value),
    };
    for (choose_add, expected) in [(true, 8_u128), (false, 14)] {
        let measured = interpret_verified_artifact(
            &verified,
            &[TerminalScalarValue::Boolean(choose_add), integer(7)],
        )
        .expect("computed conditional edge should interpret");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(integer(expected))
        );
        assert_eq!(measured.usage().total_units(), 5);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("computed conditional edge should cross the Omega abstract boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("computed conditional edge should select for both native targets");
        let assigned = assign_registers(&target_operations)
            .expect("computed conditional-edge parameter homes should assign");
        let machine_code = emit_machine_code(&assigned)
            .expect("computed conditional-edge machine code should emit");
        assert!(!machine_code.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_short_circuit_guard_keeps_computed_bindings_arm_local() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("short-circuit computed-edge source canary should compile");
    let lowered = lower_machine(&checked, "terminal_short_circuit_edge_expression")
        .expect("short-circuit guards should route into selected binding blocks");
    drop(checked);

    let machine = &lowered.semantic_module.machines[0];
    assert_eq!(machine.blocks.len(), 5);
    assert!(matches!(
        machine.blocks[0].terminator,
        Terminator::Conditional { .. }
    ));
    assert!(matches!(
        &machine.blocks[3].operations[..],
        [
            psi_terminal::Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            psi_terminal::Operation {
                kind: OperationKind::WrappingIntegerAdd { .. },
                ..
            },
        ]
    ));
    assert!(matches!(
        &machine.blocks[4].operations[..],
        [
            psi_terminal::Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            psi_terminal::Operation {
                kind: OperationKind::WrappingIntegerMultiply { .. },
                ..
            },
        ]
    ));

    let semantic_bytes =
        encode_module(&lowered.semantic_module).expect("short-circuit computed edge should encode");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("short-circuit computed-edge proof should encode");
    let semantic_module =
        decode_module(&semantic_bytes).expect("short-circuit computed edge should decode");
    let proof_bundle =
        decode_proof_bundle(&proof_bytes).expect("short-circuit computed-edge proof should decode");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("short-circuit computed edge should verify after frontend drop");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("short-circuit computed edge should have fixed fuel")
            .ceiling_units(),
        6
    );

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: u8_type,
        value: IntegerValue::Unsigned(value),
    };
    for (first, second, expected, units) in [
        (false, true, 14_u128, 5),
        (true, false, 14, 6),
        (true, true, 8, 6),
    ] {
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
                integer(7),
            ],
        )
        .expect("short-circuit computed edge should interpret");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(integer(expected))
        );
        assert_eq!(measured.usage().total_units(), units);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("short-circuit computed edge should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("short-circuit computed edge should select for both native targets");
        let assigned = assign_registers(&target_operations)
            .expect("short-circuit computed-edge homes should assign");
        let machine_code = emit_machine_code(&assigned)
            .expect("short-circuit computed-edge machine code should emit");
        assert!(!machine_code.functions[0].bytes.is_empty());
    }
}

#[cfg(unix)]
#[test]
fn checked_source_literal_conditional_emits_only_its_selected_arm() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi literal conditional source canary should compile");
    let lowered = lower_machine(&checked, "terminal_literal_conditional")
        .expect("literal source conditional should lower");
    drop(checked);

    let verified = verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("literal source conditional should verify");
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let measured = interpret_verified_artifact(
        &verified,
        &[TerminalScalarValue::Integer {
            scalar_type: u8_type,
            value: IntegerValue::Unsigned(17),
        }],
    )
    .expect("literal conditional should interpret");
    assert_eq!(measured.usage().total_units(), 5);
    assert!(
        measured
            .usage()
            .at(FuelChargeSite::Edge(EdgeId::new(1).unwrap()))
            .is_some()
    );
    assert_eq!(
        measured
            .usage()
            .at(FuelChargeSite::Edge(EdgeId::new(2).unwrap())),
        None
    );
    assert_eq!(
        measured.value(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
            scalar_type: u8_type,
            value: IntegerValue::Unsigned(20),
        })
    );

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("literal conditional should cross the Omega abstract boundary");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("literal conditional should select its known arm");
    let function = &target_operations.functions[0];
    assert_eq!(
        function.provenance.edges,
        [EdgeId::new(1).unwrap(), EdgeId::new(3).unwrap()]
    );
    assert!(matches!(
        function.operation,
        TargetOperation::ReturnIntegerExpression {
            psi_edge,
            expression: TargetIntegerExpression::WrappingAdd { .. },
            ..
        } if psi_edge == EdgeId::new(3).unwrap()
    ));
    let assigned = assign_registers(&target_operations)
        .expect("literal conditional parameter homes should assign");
    let machine_code =
        emit_machine_code(&assigned).expect("literal conditional machine code should emit");
    assert_eq!(
        run_host_machine_code_with_nine_u8(&machine_code.functions[0].bytes, 17, 0, 0),
        20
    );
}

#[test]
fn checked_source_boolean_conditional_reaches_native_control() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi Boolean conditional source canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_conditional")
        .expect("Boolean source conditional should lower");
    drop(checked);

    let verified = verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("Boolean source conditional should verify after frontend drop");
    let fixed = derive_fixed_entry_fuel(&verified, MachineId::new(1).unwrap())
        .expect("Boolean source conditional should have an exact fuel bound");
    assert_eq!(fixed.ceiling_units(), 2);
    for (condition, expected) in [(true, true), (false, false)] {
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(condition),
                TerminalScalarValue::Boolean(true),
                TerminalScalarValue::Boolean(false),
            ],
        )
        .expect("Boolean source conditional should interpret");
        assert_eq!(measured.usage().total_units(), 2);
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("Boolean source conditional should cross the Omega boundary");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("Boolean source conditional should lower for the host");
    assert!(matches!(
        target_operations.functions[0].operation,
        TargetOperation::ReturnBooleanConditionalControl { .. }
    ));
    let assigned = assign_registers(&target_operations)
        .expect("Boolean source conditional parameter homes should assign");
    let _machine_code =
        emit_machine_code(&assigned).expect("Boolean source conditional machine code should emit");
    #[cfg(unix)]
    for (condition, expected) in [(true, 1), (false, 0)] {
        assert_eq!(
            run_host_machine_code_with_conditional_u8(
                &machine_code.functions[0].bytes,
                condition,
                1,
                0,
            ),
            expected
        );
    }
}

#[cfg(unix)]
#[test]
fn checked_source_boolean_conditional_arms_preserve_short_circuit_control() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi Boolean conditional source canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_conditional_short_circuit_arms")
        .expect("Boolean conditional short-circuit arms should lower");
    drop(checked);

    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("Boolean conditional arm control should encode");
    let semantic_module =
        decode_module(&semantic_bytes).expect("Boolean conditional arm control should decode");
    let verified = verify_module(
        &semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("Boolean conditional arm control should verify");
    let fixed = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("Boolean conditional arm control should have exact fuel");
    assert_eq!(fixed.ceiling_units(), 6);

    for (condition, when_true, when_false) in [
        (false, false, false),
        (false, false, true),
        (false, true, false),
        (false, true, true),
        (true, false, false),
        (true, false, true),
        (true, true, false),
        (true, true, true),
    ] {
        let expected_units = if (condition && !when_true) || (!condition && when_false) {
            4
        } else {
            6
        };
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(condition),
                TerminalScalarValue::Boolean(when_true),
                TerminalScalarValue::Boolean(when_false),
            ],
        )
        .expect("Boolean conditional arm control should interpret");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(!condition))
        );
        assert_eq!(measured.usage().total_units(), expected_units);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("Boolean conditional arm control should cross the Omega boundary");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("Boolean conditional arm control should lower for the host");
    assert!(matches!(
        target_operations.functions[0].operation,
        TargetOperation::ReturnBooleanConditionalControl { .. }
    ));
    let assigned = assign_registers(&target_operations)
        .expect("Boolean conditional arm control homes should assign");
    let machine_code =
        emit_machine_code(&assigned).expect("Boolean conditional arm control should emit");
    let object_artifact = build_object_artifact(&machine_code)
        .expect("Boolean conditional arm control should form an object");
    let entry = object_artifact.entry_function();
    for (condition, when_true, when_false) in [
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
                condition,
                when_true,
                when_false,
            ),
            i32::from(!condition)
        );
    }
}

#[cfg(unix)]
#[test]
fn checked_source_boolean_conditional_guard_preserves_short_circuit_control() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi Boolean conditional source canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_conditional_short_circuit_guard")
        .expect("Boolean conditional short-circuit guard should lower");
    drop(checked);

    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("Boolean conditional guard control should encode");
    let semantic_module =
        decode_module(&semantic_bytes).expect("Boolean conditional guard control should decode");
    let verified = verify_module(
        &semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("Boolean conditional guard control should verify");
    let fixed = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("Boolean conditional guard control should have exact fuel");
    assert_eq!(fixed.ceiling_units(), 3);

    for (first, second, fallback) in [
        (false, false, false),
        (false, false, true),
        (false, true, false),
        (false, true, true),
        (true, false, false),
        (true, false, true),
        (true, true, false),
        (true, true, true),
    ] {
        let expected = if first && second { first } else { fallback };
        let expected_units = if first { 3 } else { 2 };
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
                TerminalScalarValue::Boolean(fallback),
            ],
        )
        .expect("Boolean conditional guard control should interpret");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
        assert_eq!(measured.usage().total_units(), expected_units);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("Boolean conditional guard control should cross the Omega boundary");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("Boolean conditional guard control should lower for the host");
    assert!(matches!(
        target_operations.functions[0].operation,
        TargetOperation::ReturnBooleanConditionalControl { .. }
    ));
    let assigned = assign_registers(&target_operations)
        .expect("Boolean conditional guard control homes should assign");
    let machine_code =
        emit_machine_code(&assigned).expect("Boolean conditional guard control should emit");
    let object_artifact = build_object_artifact(&machine_code)
        .expect("Boolean conditional guard control should form an object");
    let entry = object_artifact.entry_function();
    for (first, second, fallback) in [
        (false, false, false),
        (false, false, true),
        (false, true, false),
        (false, true, true),
        (true, false, false),
        (true, false, true),
        (true, true, false),
        (true, true, true),
    ] {
        let expected = if first && second { first } else { fallback };
        assert_eq!(
            run_host_machine_code_with_three_bools(
                entry.bytes(&object_artifact),
                first,
                second,
                fallback,
            ),
            i32::from(expected)
        );
    }
}

#[test]
fn checked_source_nested_boolean_control_reaches_both_native_targets() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("nested Boolean source canary should compile");
    let lowered = lower_machine(&checked, "terminal_nested_boolean_control")
        .expect("rooted acyclic Boolean control should lower");
    drop(checked);

    assert_eq!(lowered.semantic_module.machines[0].blocks.len(), 13);
    let semantic_bytes =
        encode_module(&lowered.semantic_module).expect("nested Boolean control should encode");
    let proof_bytes =
        encode_proof_bundle(&lowered.proof_bundle).expect("nested Boolean proof should encode");
    let semantic_module =
        decode_module(&semantic_bytes).expect("nested Boolean control should decode");
    let proof_bundle =
        decode_proof_bundle(&proof_bytes).expect("nested Boolean proof should decode");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("nested Boolean control should verify after frontend drop");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("nested Boolean control should have fixed fuel")
            .ceiling_units(),
        12
    );

    for (arguments, expected, units) in [
        ([true, true, true, true, false, false], true, 6),
        ([true, false, true, true, false, false], true, 8),
        ([false, false, true, true, false, true], true, 7),
        ([true, true, false, true, false, false], true, 10),
    ] {
        let arguments = arguments.map(TerminalScalarValue::Boolean);
        let measured = interpret_verified_artifact(&verified, &arguments)
            .expect("nested Boolean control should interpret");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
        assert_eq!(measured.usage().total_units(), units);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("nested Boolean control should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("nested Boolean control should select for both native targets");
        assert!(matches!(
            target_operations.functions[0].operation,
            TargetOperation::ReturnBooleanConditionalControl { .. }
        ));
        let assigned =
            assign_registers(&target_operations).expect("nested Boolean homes should assign");
        let machine_code =
            emit_machine_code(&assigned).expect("nested Boolean control should emit");
        assert!(!machine_code.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_short_circuit_tuple_binding_is_staged_left_to_right() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("short-circuit tuple-binding source canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_chain_short_circuit_tuple")
        .expect("short-circuit tuple bindings should lower in ordered stages");
    drop(checked);

    assert_eq!(lowered.semantic_module.machines[0].blocks.len(), 14);
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("short-circuit tuple binding should encode canonically");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("short-circuit tuple-binding proof should encode canonically");
    let semantic_module =
        decode_module(&semantic_bytes).expect("short-circuit tuple binding should decode");
    let proof_bundle =
        decode_proof_bundle(&proof_bytes).expect("short-circuit tuple-binding proof should decode");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("short-circuit tuple binding should verify after frontend drop");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("short-circuit tuple binding should have exact fuel")
            .ceiling_units(),
        13
    );

    for (arguments, expected, units) in [
        ([true, false, false, true], false, 11),
        ([false, true, false, false], false, 12),
        ([true, false, true, true], true, 12),
        ([false, false, true, false], true, 13),
    ] {
        let measured =
            interpret_verified_artifact(&verified, &arguments.map(TerminalScalarValue::Boolean))
                .expect("short-circuit tuple binding should interpret");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
        assert_eq!(measured.usage().total_units(), units);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("short-circuit tuple binding should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("short-circuit tuple binding should select for both native targets");
        assert!(matches!(
            target_operations.functions[0].operation,
            TargetOperation::ReturnBooleanConditionalControl { .. }
        ));
        let assigned = assign_registers(&target_operations)
            .expect("short-circuit tuple-binding homes should assign");
        let machine_code = emit_machine_code(&assigned)
            .expect("short-circuit tuple-binding machine code should emit");
        assert!(!machine_code.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_boolean_conditional_edges_compute_only_on_the_selected_arm() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("computed Boolean conditional-edge source canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_computed_conditional_edges")
        .expect("computed Boolean conditional edges should lower into selected-arm blocks");
    drop(checked);

    assert_eq!(lowered.semantic_module.machines[0].blocks.len(), 17);
    let semantic_bytes = encode_module(&lowered.semantic_module)
        .expect("computed Boolean conditional edges should encode canonically");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("computed Boolean conditional-edge proof should encode canonically");
    let semantic_module = decode_module(&semantic_bytes)
        .expect("computed Boolean conditional-edge module should decode");
    let proof_bundle = decode_proof_bundle(&proof_bytes)
        .expect("computed Boolean conditional-edge proof should decode");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("computed Boolean conditional edges should verify after frontend drop");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("computed Boolean conditional edges should have exact fuel")
            .ceiling_units(),
        14
    );

    for (arguments, expected, units) in [
        ([false, true, true, true, false], false, 6),
        ([true, false, true, true, true], false, 7),
        ([true, true, false, true, false], false, 12),
        ([true, true, true, true, false], true, 13),
    ] {
        let measured =
            interpret_verified_artifact(&verified, &arguments.map(TerminalScalarValue::Boolean))
                .expect("computed Boolean conditional edges should interpret");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
        assert_eq!(measured.usage().total_units(), units);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("computed Boolean conditional edges should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("computed Boolean conditional edges should select for both native targets");
        assert!(matches!(
            target_operations.functions[0].operation,
            TargetOperation::ReturnBooleanConditionalControl { .. }
        ));
        let assigned = assign_registers(&target_operations)
            .expect("computed Boolean conditional-edge homes should assign");
        let machine_code = emit_machine_code(&assigned)
            .expect("computed Boolean conditional-edge machine code should emit");
        assert!(!machine_code.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_mixed_scalar_boolean_graph_uses_the_typed_dag() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("mixed-scalar Boolean source canary should compile");
    let lowered = lower_machine(&checked, "terminal_mixed_scalar_boolean_graph")
        .expect("mixed-scalar Boolean graph should lower through terminal Psi");
    drop(checked);

    let semantic_bytes =
        encode_module(&lowered.semantic_module).expect("mixed-scalar Boolean graph should encode");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("mixed-scalar Boolean proof should encode");
    let semantic_module =
        decode_module(&semantic_bytes).expect("mixed-scalar Boolean graph should decode");
    let proof_bundle =
        decode_proof_bundle(&proof_bytes).expect("mixed-scalar Boolean proof should decode");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("mixed-scalar Boolean graph should verify after frontend drop");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, semantic_module.entry)
            .expect("mixed-scalar Boolean graph should have fixed fuel")
            .ceiling_units(),
        9
    );

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: u8_type,
        value: IntegerValue::Unsigned(value),
    };
    for (choose_less, left, right, expected) in [
        (true, 1, 2, true),
        (true, 5, 2, false),
        (false, 3, 2, true),
        (false, 1, 2, false),
    ] {
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(choose_less),
                integer(left),
                integer(right),
            ],
        )
        .expect("mixed-scalar Boolean graph should interpret");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
        assert_eq!(measured.usage().total_units(), 9);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("mixed-scalar Boolean graph should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("mixed-scalar Boolean graph should select for both native targets");
        assert!(matches!(
            target_operations.functions[0].operation,
            TargetOperation::ReturnBooleanConditionalControl { .. }
        ));
        let assigned =
            assign_registers(&target_operations).expect("mixed-scalar Boolean homes should assign");
        let machine_code =
            emit_machine_code(&assigned).expect("mixed-scalar Boolean graph should emit");
        assert!(!machine_code.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_mixed_scalar_boolean_short_circuit_preserves_selected_fuel() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("mixed-scalar Boolean short-circuit canary should compile");
    let lowered = lower_machine(&checked, "terminal_mixed_scalar_boolean_short_circuit")
        .expect("mixed-scalar Boolean short-circuit graph should lower");
    drop(checked);

    let verified = verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("mixed-scalar Boolean short-circuit graph should verify");
    assert_eq!(
        derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
            .expect("mixed-scalar Boolean short-circuit graph should have fixed fuel")
            .ceiling_units(),
        15
    );
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: u8_type,
        value: IntegerValue::Unsigned(value),
    };
    for (first, second, value, limit, expected, expected_units) in [
        (false, true, 1, 4, false, 12),
        (true, false, 1, 4, false, 13),
        (true, true, 1, 4, true, 15),
        (true, true, 4, 4, false, 15),
    ] {
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
                integer(value),
                integer(limit),
            ],
        )
        .expect("mixed-scalar Boolean short-circuit graph should interpret");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
        assert_eq!(measured.usage().total_units(), expected_units);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("mixed-scalar Boolean short-circuit graph should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("mixed-scalar Boolean short-circuit graph should select natively");
        let assigned = assign_registers(&target_operations)
            .expect("mixed-scalar Boolean short-circuit homes should assign");
        assert!(
            !emit_machine_code(&assigned)
                .expect("mixed-scalar Boolean short-circuit graph should emit")
                .functions[0]
                .bytes
                .is_empty()
        );
    }
}

#[cfg(unix)]
#[test]
fn source_closed_integer_chain_matches_emitted_host_machine_code() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi closed integer-chain canary should compile");
    let lowered = lower_machine(&checked, "terminal_closed_integer_chain")
        .expect("closed integer state chain should lower");
    drop(checked);

    let verified = verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("closed integer state chain should verify");
    let abstract_operations = lower_verified_artifact(&verified)
        .expect("closed integer state chain should lower without frontend state");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("closed integer state chain should select for the host");
    let assigned =
        assign_registers(&target_operations).expect("closed chain target homes should assign");
    let machine_code =
        emit_machine_code(&assigned).expect("closed integer state chain should emit");
    let object_artifact = build_object_artifact(&machine_code)
        .expect("closed integer state chain should form an object");
    let entry = object_artifact.entry_function();
    assert_eq!(entry.provenance.operations.len(), 5);
    assert_eq!(entry.provenance.edges.len(), 3);
    assert_eq!(run_host_machine_code(entry.bytes(&object_artifact)), 42);
}

#[cfg(unix)]
#[test]
fn source_runtime_arithmetic_combines_register_and_stack_parameters() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi runtime arithmetic source canary should compile");
    let lowered = [
        (
            "terminal_runtime_wrapping_add",
            100_u8,
            2_u8,
            200_u8,
            44_i32,
            1_usize,
        ),
        ("terminal_runtime_nested_wrapping", 100, 3, 200, 132, 2),
        ("terminal_runtime_jump_wrapping", 5, 2, 40, 135, 3),
        ("terminal_runtime_chain_wrapping", 5, 2, 40, 134, 5),
        ("terminal_runtime_multi_binding", 5, 2, 40, 137, 7),
    ]
    .into_iter()
    .map(
        |(machine, first, second, ninth, expected, operation_count)| {
            (
                machine,
                first,
                second,
                ninth,
                expected,
                operation_count,
                lower_machine(&checked, machine)
                    .unwrap_or_else(|error| panic!("{machine} should lower: {error:?}")),
            )
        },
    )
    .collect::<Vec<_>>();
    drop(checked);

    for (machine, first, second, ninth, expected, operation_count, lowered) in lowered {
        let verified = verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .unwrap_or_else(|error| panic!("{machine} terminal Psi should verify: {error:?}"));
        let abstract_operations = lower_verified_artifact(&verified)
            .unwrap_or_else(|error| panic!("{machine} should lower: {error:?}"));
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .unwrap_or_else(|error| panic!("{machine} should select: {error:?}"));
        let assigned =
            assign_registers(&target_operations).expect("integer target homes should assign");
        let machine_code = emit_machine_code(&assigned)
            .unwrap_or_else(|error| panic!("{machine} should emit: {error:?}"));
        let object_artifact = build_object_artifact(&machine_code)
            .unwrap_or_else(|error| panic!("{machine} should form an object: {error:?}"));
        let entry = object_artifact.entry_function();
        assert_eq!(entry.provenance.operations.len(), operation_count);
        assert_eq!(
            run_host_machine_code_with_nine_u8(entry.bytes(&object_artifact), first, second, ninth,),
            expected,
            "{machine} native result"
        );
    }
}
