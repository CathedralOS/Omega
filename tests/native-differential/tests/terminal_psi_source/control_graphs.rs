use super::*;
use compiler::CheckedCompileRequest;

#[test]
fn checked_source_nested_jump_expressions_reach_terminal_and_target_lowering() {
    let checked = compile_to_checked(CheckedCompileRequest::new(&source_canary(), None))
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
        let _target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("computed nested jump should select for both native targets");
    }
}

#[test]
fn checked_source_conditional_edge_expressions_execute_only_on_the_selected_arm() {
    let checked = compile_to_checked(CheckedCompileRequest::new(&source_canary(), None))
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
            terminal_psi::Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            terminal_psi::Operation {
                kind: OperationKind::WrappingIntegerAdd { .. },
                ..
            },
        ]
    ));
    assert!(matches!(
        &machine.blocks[3].operations[..],
        [
            terminal_psi::Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            terminal_psi::Operation {
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
        let _target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("computed conditional edge should select for both native targets");
    }
}

#[test]
fn checked_source_short_circuit_guard_keeps_computed_bindings_arm_local() {
    let checked = compile_to_checked(CheckedCompileRequest::new(&source_canary(), None))
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
            terminal_psi::Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            terminal_psi::Operation {
                kind: OperationKind::WrappingIntegerAdd { .. },
                ..
            },
        ]
    ));
    assert!(matches!(
        &machine.blocks[4].operations[..],
        [
            terminal_psi::Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            terminal_psi::Operation {
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
        let _target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("short-circuit computed edge should select for both native targets");
    }
}

#[test]
fn checked_source_mixed_scalar_boolean_short_circuit_preserves_selected_fuel() {
    let checked = compile_to_checked(CheckedCompileRequest::new(&source_canary(), None))
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
        let _target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("mixed-scalar Boolean short-circuit graph should select natively");
    }
}

#[cfg(unix)]
#[test]
fn source_closed_integer_chain_matches_target_lowering() {
    let checked = compile_to_checked(CheckedCompileRequest::new(&source_canary(), None))
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
    let _target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("closed integer state chain should select for the host");
}

#[test]
fn source_runtime_arithmetic_retains_target_parameter_abi_and_provenance() {
    let checked = compile_to_checked(CheckedCompileRequest::new(&source_canary(), None))
        .expect("terminal-Psi runtime arithmetic source canary should compile");
    let lowered = [
        ("terminal_runtime_wrapping_add", 1_usize),
        ("terminal_runtime_nested_wrapping", 2),
        ("terminal_runtime_jump_wrapping", 3),
        ("terminal_runtime_chain_wrapping", 5),
        ("terminal_runtime_multi_binding", 7),
    ]
    .into_iter()
    .map(|(machine, operation_count)| {
        (
            machine,
            operation_count,
            lower_machine(&checked, machine)
                .unwrap_or_else(|error| panic!("{machine} should lower: {error:?}")),
        )
    })
    .collect::<Vec<_>>();
    drop(checked);

    // runtime_policy_and_narrowing::checked_source_runtime_integer_policy_operations_survive_frontend_drop
    // owns exact inputs, results, and fuel. This test owns target ABI and
    // source custody; successful lowering alone does not establish execution.
    for (machine, operation_count, lowered) in lowered {
        let verified = verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .unwrap_or_else(|error| panic!("{machine} terminal Psi should verify: {error:?}"));
        let abstract_operations = lower_verified_artifact(&verified)
            .unwrap_or_else(|error| panic!("{machine} should lower: {error:?}"));
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            let target_operations = lower_to_target_operations(&abstract_operations, target)
                .unwrap_or_else(|error| {
                    panic!("{machine} should select for {target:?}: {error:?}")
                });
            let [function] = target_operations.functions.as_slice() else {
                panic!("{machine} must retain one target function");
            };
            assert_eq!(
                function.provenance.operations.len(),
                operation_count,
                "{machine}"
            );
            let abi = function.scalar_abi.as_ref().expect("source scalar ABI");
            assert_eq!(abi.parameters.len(), 9);
            for (parameter, source) in abi
                .parameters
                .iter()
                .zip(&abstract_operations.functions[0].parameters)
            {
                assert_eq!(parameter.value, source.value);
                assert_eq!(parameter.scalar_type, source.scalar_type);
            }
            assert!(matches!(
                abi.parameters[0].placement.locations.as_slice(),
                [calling_conventions::ValueLocation::Register { .. }]
            ));
            assert!(matches!(
                abi.parameters[8].placement.locations.as_slice(),
                [calling_conventions::ValueLocation::Stack { .. }]
            ));
        }
    }
}
