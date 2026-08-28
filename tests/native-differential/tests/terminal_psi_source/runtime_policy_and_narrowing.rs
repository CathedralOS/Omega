use super::*;

#[cfg(unix)]
#[test]
fn source_wrapping_add_matches_emitted_host_machine_code() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi integer policy source canary should compile");
    let lowered = lower_machine(&checked, "terminal_wrapping_add")
        .expect("source wrapping add should lower to terminal Psi");
    drop(checked);

    let verified = verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source wrapping add terminal Psi should verify");
    let abstract_operations = lower_verified_artifact(&verified)
        .expect("verified source wrapping add should lower without frontend state");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("source wrapping add should select for the host");
    let assigned = assign_registers(&target_operations).expect("source target homes should assign");
    let machine_code =
        emit_machine_code(&assigned).expect("source wrapping add machine code should emit");
    let object_artifact = build_object_artifact(&machine_code)
        .expect("source wrapping add should form an owned object artifact");
    let entry = object_artifact.entry_function();
    assert_eq!(
        entry.provenance.operations,
        [
            OperationId::new(1).expect("jump constant"),
            OperationId::new(2).expect("right constant"),
            OperationId::new(3).expect("wrapping add"),
        ]
    );
    assert_eq!(run_host_machine_code(entry.bytes(&object_artifact)), 44);
}

#[cfg(unix)]
#[test]
fn checked_source_ninth_parameter_reaches_the_host_stack_abi() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi runtime-parameter source canary should compile");
    let lowered = lower_machine(&checked, "terminal_ninth_parameter")
        .expect("nine-parameter source machine should lower to terminal Psi");
    drop(checked);

    let verified = verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source-produced nine-parameter terminal Psi should verify");
    let fixed_fuel = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("direct parameter return should have fixed fuel");
    assert_eq!(fixed_fuel.ceiling_units(), 1);
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let arguments = [1_u128, 2, 3, 4, 5, 6, 7, 8, 77]
        .into_iter()
        .map(|value| TerminalScalarValue::Integer {
            scalar_type: u8_type,
            value: IntegerValue::Unsigned(value),
        })
        .collect::<Vec<_>>();
    let measured = interpret_verified_artifact(&verified, &arguments)
        .expect("source-produced ninth parameter should execute");
    assert_eq!(measured.usage().total_units(), 1);
    assert_eq!(
        measured.value(),
        TerminalExecutionResult::Scalar(arguments[8])
    );

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("verified source parameters should lower without frontend state");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("source parameters should select host ABI locations");
    let assigned =
        assign_registers(&target_operations).expect("parameter target homes should assign");
    let machine_code = emit_machine_code(&assigned).expect("source parameter return should emit");
    let object_artifact = build_object_artifact(&machine_code)
        .expect("source parameter return should form an object artifact");
    let entry = object_artifact.entry_function();
    assert!(entry.provenance.operations.is_empty());
    assert_eq!(
        entry.provenance.edges,
        [EdgeId::new(1).expect("return edge")]
    );
    assert_eq!(
        run_host_machine_code_with_nine_u8(entry.bytes(&object_artifact), 1, 2, 77),
        77
    );
}

#[test]
fn checked_source_runtime_integer_policy_operations_survive_frontend_drop() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi runtime arithmetic source canary should compile");
    let cases = [
        ("terminal_direct_integer_constant", vec![], 42_u128, 2_u64),
        ("terminal_exact_literal_narrowing", vec![], 127_u128, 2_u64),
        ("terminal_closed_integer_chain", vec![], 42_u128, 8_u64),
        (
            "terminal_runtime_wrapping_add",
            vec![100_u128, 2, 3, 4, 5, 6, 7, 8, 200],
            44_u128,
            2_u64,
        ),
        (
            "terminal_runtime_nested_wrapping",
            vec![100_u128, 3, 3, 4, 5, 6, 7, 8, 200],
            132,
            3,
        ),
        (
            "terminal_runtime_jump_wrapping",
            vec![5_u128, 2, 3, 4, 5, 6, 7, 8, 40],
            135,
            5,
        ),
        (
            "terminal_runtime_chain_wrapping",
            vec![5_u128, 2, 3, 4, 5, 6, 7, 8, 40],
            134,
            8,
        ),
        (
            "terminal_runtime_multi_binding",
            vec![5_u128, 2, 3, 4, 5, 6, 7, 8, 40],
            137,
            10,
        ),
        ("terminal_runtime_saturating_add", vec![200], 255, 3),
        ("terminal_runtime_wrapping_subtract", vec![5], 251, 3),
        ("terminal_runtime_saturating_subtract", vec![5], 0, 3),
        ("terminal_runtime_wrapping_multiply", vec![20], 4, 3),
        ("terminal_runtime_saturating_multiply", vec![20], 255, 3),
    ];
    let lowered = cases
        .into_iter()
        .map(|(machine, arguments, expected, fuel)| {
            (
                machine,
                arguments,
                expected,
                fuel,
                lower_machine(&checked, machine)
                    .unwrap_or_else(|error| panic!("{machine} should lower: {error:?}")),
            )
        })
        .collect::<Vec<_>>();
    drop(checked);

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    for (machine, arguments, expected, fuel, lowered) in lowered {
        let verified = verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .unwrap_or_else(|error| panic!("{machine} terminal Psi should verify: {error:?}"));
        let fixed_fuel = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
            .unwrap_or_else(|error| panic!("{machine} should have fixed fuel: {error:?}"));
        assert_eq!(fixed_fuel.ceiling_units(), fuel, "{machine} fuel");
        let arguments = arguments
            .into_iter()
            .map(|value| TerminalScalarValue::Integer {
                scalar_type: u8_type,
                value: IntegerValue::Unsigned(value),
            })
            .collect::<Vec<_>>();
        let measured = interpret_verified_artifact(&verified, &arguments)
            .unwrap_or_else(|error| panic!("{machine} should execute: {error:?}"));
        assert_eq!(measured.usage().total_units(), fuel, "{machine} usage");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                scalar_type: u8_type,
                value: IntegerValue::Unsigned(expected),
            }),
            "{machine} result"
        );
    }
}

#[test]
fn checked_source_exact_literal_narrowing_relands_before_psi() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("exact literal narrowing source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_literal_narrowing")
        .expect("exact literal narrowing should lower to terminal Psi");
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");

    let operations = &lowered.semantic_module.machines[0].blocks[0].operations;
    assert_eq!(operations.len(), 1);
    assert!(matches!(
        operations[0].kind,
        OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(127)
        }
    ));
    assert_eq!(
        operations[0].result.expect_scalar().scalar_type,
        ScalarType::Integer(u8_type)
    );

    let semantic = encode_module(&lowered.semantic_module).expect("narrowing semantic bytes");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("narrowing proof bytes");
    drop(lowered);
    let measured =
        interpret_terminal_artifact_measured(&semantic, &proof, &AdmissionProfile::default(), &[])
            .expect("decoded narrowing artifact should interpret");
    assert_eq!(
        measured.value(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
            scalar_type: u8_type,
            value: IntegerValue::Unsigned(127),
        })
    );
    assert_eq!(measured.usage().total_units(), 2);

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("narrowing artifact should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("narrowing constant should select");
        let assigned = assign_registers(&target_operations).expect("narrowing homes should assign");
        emit_machine_code(&assigned).expect("narrowing constant should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("narrowing host selection");
        let assigned = assign_registers(&target_operations).expect("narrowing host homes");
        let machine_code = emit_machine_code(&assigned).expect("narrowing host emission");
        let object = build_object_artifact(&machine_code).expect("narrowing host object");
        assert_eq!(
            run_host_machine_code(object.entry_function().bytes(&object)),
            127
        );
    }
}
