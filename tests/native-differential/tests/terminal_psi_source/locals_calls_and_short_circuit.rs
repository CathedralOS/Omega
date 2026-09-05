use super::*;

#[test]
fn checked_source_integer_policy_operations_survive_frontend_drop() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi integer policy source canary should compile");
    let cases = [
        ("terminal_wrapping_add", 44_u128),
        ("terminal_saturating_add", 255),
        ("terminal_wrapping_subtract", 251),
        ("terminal_saturating_subtract", 0),
        ("terminal_wrapping_multiply", 4),
        ("terminal_saturating_multiply", 255),
    ];
    let lowered = cases
        .iter()
        .map(|(machine, expected)| {
            (
                *machine,
                *expected,
                lower_machine(&checked, machine)
                    .unwrap_or_else(|error| panic!("{machine} should lower: {error:?}")),
            )
        })
        .collect::<Vec<_>>();
    drop(checked);

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    for (machine, expected, lowered) in lowered {
        let verified = verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .unwrap_or_else(|error| panic!("{machine} terminal Psi should verify: {error:?}"));
        let fixed_fuel = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
            .unwrap_or_else(|error| panic!("{machine} should have fixed fuel: {error:?}"));
        assert_eq!(fixed_fuel.ceiling_units(), 5, "{machine} fuel");
        let measured = interpret_verified_artifact(&verified, &[])
            .unwrap_or_else(|error| panic!("{machine} should execute: {error:?}"));
        assert_eq!(measured.usage().total_units(), 5, "{machine} usage");
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
fn checked_source_scalar_locals_become_terminal_block_values() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi scalar-local source canary should compile");
    let lowered = lower_machine(&checked, "terminal_scalar_locals")
        .expect("immutable scalar locals should lower from checked plans");
    let mut without_typed_frontend = checked.clone();
    without_typed_frontend.typed = Default::default();
    let without_typed_frontend = lower_machine(&without_typed_frontend, "terminal_scalar_locals")
        .expect("scalar locals must survive complete typed-tree disposal");
    assert_eq!(without_typed_frontend, lowered);
    drop(checked);

    let semantic = encode_module(&lowered.semantic_module)
        .expect("scalar-local terminal Psi should encode canonically");
    let proof = encode_proof_bundle(&lowered.proof_bundle)
        .expect("scalar-local proof bundle should encode canonically");
    let debug = encode_debug_map(
        &lowered.semantic_module,
        lowered
            .debug_map
            .as_ref()
            .expect("scalar-local lowering should retain presentation spans"),
    )
    .expect("scalar-local debug map should encode canonically");
    let semantic = decode_module(&semantic).expect("scalar-local terminal Psi should decode");
    let proof = decode_proof_bundle(&proof).expect("scalar-local proof bundle should decode");
    let debug = decode_debug_map(&semantic, &debug).expect("scalar-local debug map should decode");
    let verified = verify_module(&semantic, &proof, &AdmissionProfile::default())
        .expect("scalar-local terminal Psi should verify after frontend disposal");

    let block = &semantic.machines[0].blocks[0];
    assert_eq!(block.operations.len(), 4);
    assert!(matches!(
        block.operations[1].kind,
        OperationKind::WrappingIntegerAdd { .. }
    ));
    assert!(matches!(
        block.operations[3].kind,
        OperationKind::WrappingIntegerMultiply { .. }
    ));
    assert_eq!(
        debug
            .sites
            .iter()
            .filter(|site| matches!(site.subject, DebugSubject::Operation(_)))
            .count(),
        4
    );

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let measured = interpret_verified_artifact(
        &verified,
        &[TerminalScalarValue::Integer {
            scalar_type: u8_type,
            value: IntegerValue::Unsigned(20),
        }],
    )
    .expect("scalar-local terminal Psi should execute");
    assert_eq!(measured.usage().total_units(), 5);
    assert_eq!(
        measured.value(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
            scalar_type: u8_type,
            value: IntegerValue::Unsigned(50),
        })
    );

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("verified scalar locals should lower without frontend state");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("scalar locals should lower for both native targets");
        let assigned = assign_registers(&target_operations)
            .expect("scalar-local target operations should assign");
        let emitted =
            emit_machine_code(&assigned).expect("scalar-local target operations should emit");
        assert!(!emitted.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_boolean_local_becomes_a_terminal_block_value() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi Boolean-local source canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_local")
        .expect("an immutable Boolean local should lower from checked plans");
    drop(checked);

    let semantic = encode_module(&lowered.semantic_module)
        .expect("Boolean-local terminal Psi should encode canonically");
    let proof = encode_proof_bundle(&lowered.proof_bundle)
        .expect("Boolean-local proof bundle should encode canonically");
    let semantic = decode_module(&semantic).expect("Boolean-local terminal Psi should decode");
    let proof = decode_proof_bundle(&proof).expect("Boolean-local proof bundle should decode");
    let verified = verify_module(&semantic, &proof, &AdmissionProfile::default())
        .expect("Boolean-local terminal Psi should verify after frontend disposal");

    assert_eq!(semantic.machines[0].blocks[0].operations.len(), 1);
    assert!(matches!(
        semantic.machines[0].blocks[0].operations[0].kind,
        OperationKind::BooleanNot { .. }
    ));
    let measured = interpret_verified_artifact(&verified, &[TerminalScalarValue::Boolean(false)])
        .expect("Boolean-local terminal Psi should execute");
    assert_eq!(measured.usage().total_units(), 2);
    assert_eq!(
        measured.value(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
    );

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("verified Boolean local should lower without frontend state");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("Boolean local should lower for both native targets");
        let assigned = assign_registers(&target_operations)
            .expect("Boolean-local target operations should assign");
        let emitted =
            emit_machine_code(&assigned).expect("Boolean-local target operations should emit");
        assert!(!emitted.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_direct_call_emits_its_reachable_terminal_closure() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("the direct-call source canary should compile");
    let lowered = lower_machine(&checked, "terminal_call_forward")
        .expect("checked direct calls should compose a terminal machine closure");
    assert_eq!(lowered.semantic_module.machines.len(), 2);
    let call = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        .expect("the caller should contain one terminal call");
    let OperationKind::Call {
        callee,
        arguments,
        requirement_obligations,
        crash_continuations,
    } = &call.kind
    else {
        unreachable!()
    };
    assert_eq!(*callee, MachineId::new(2).expect("callee identity"));
    assert_eq!(arguments.len(), 1);
    assert_eq!(requirement_obligations.len(), 1);
    assert!(crash_continuations.is_empty());

    let verified = verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the source-produced direct-call closure should verify");
    for value in [false, true] {
        assert_eq!(
            interpret_verified_artifact(&verified, &[TerminalScalarValue::Boolean(value)])
                .expect("verified direct call should interpret")
                .value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(value)),
        );
    }
    let abstract_operations = lower_verified_artifact(&verified)
        .expect("the verified source call should reach Omega lowering");
    assert_eq!(abstract_operations.functions.len(), 2);
    assert!(matches!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .find(|operation| matches!(operation, AbstractOperation::Call { .. })),
        Some(AbstractOperation::Call { .. })
    ));
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("the source-produced call closure should select a native calling plan");
        let assigned = assign_registers(&target_operations)
            .expect("the source-produced call arguments should assign");
        let emitted =
            emit_machine_code(&assigned).expect("the source-produced call closure should emit");
        assert_eq!(emitted.functions.len(), 2);
        assert_eq!(emitted.functions[0].internal_calls.len(), 1);
    }
}

#[test]
fn checked_trait_operator_structural_call_reaches_native_artifact_custody() {
    let checked = compile_to_checked(
        &terminal_source_canary(fixture_roster::STRUCTURAL_SCALAR_TRAIT_OPERATOR),
        None,
    )
    .expect("the fixed trait-operator source canary should compile");
    let plan = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .trait_operator_machines
        .first()
        .expect("one exact trait-operator structural call plan");
    let name = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find(|selection| selection.machine == plan.machine)
        .expect("selected specialized caller")
        .name
        .clone();
    let lowered = lower_machine(&checked, &name)
        .expect("the fixed trait-operator call should lower to terminal Psi");
    let verified = verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the structural scalar call closure should verify");
    let abstract_operations = lower_verified_artifact(&verified)
        .expect("the structural scalar call should cross the Omega boundary");

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("the structural scalar call should select a native calling plan");
        assert!(matches!(
            target_operations.functions[0].operation,
            TargetOperation::ReturnStructuralScalarCall { .. }
        ));
        let assigned = assign_registers(&target_operations)
            .expect("the structural scalar aggregate copies should assign");
        let emitted =
            emit_machine_code(&assigned).expect("the structural scalar call closure should emit");
        let caller = &emitted.functions[0];
        let [custody] = caller.internal_unit_calls.as_slice() else {
            panic!("one structural scalar call retains artifact custody")
        };
        assert_eq!(custody.result, Some(ScalarType::Boolean));
        assert_eq!(custody.arguments.len(), 2);
        assert!(
            custody
                .arguments
                .iter()
                .all(|argument| argument.path.is_empty())
        );
        let mut erased_result = emitted.clone();
        erased_result.functions[0].internal_unit_calls[0].result = None;
        assert!(
            build_object_artifact(&erased_result).is_err(),
            "object validation must reject erasing result-bearing call custody"
        );
        let object = build_object_artifact(&emitted)
            .expect("the structural scalar call should replay at the object boundary");
        let image = emit_executable_image(&object, 3)
            .expect("the structural scalar call should link into an executable image");
        let installation = build_installation_record(
            &image,
            ProfileDecisionId::new(70).expect("installation profile decision"),
        )
        .expect("the structural scalar call should retain installation custody");
        let bytes = encode_installation_record(&installation)
            .expect("structural scalar installation should encode");
        assert_eq!(decode_installation_record(&bytes), Ok(installation));
    }
}

#[test]
fn checked_source_short_circuit_call_argument_is_staged_before_the_call() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("the short-circuit call source canary should compile");
    let lowered = lower_machine(&checked, "terminal_call_short_argument")
        .expect("a checked short-circuit call argument should lower through terminal control");
    assert_eq!(lowered.semantic_module.machines.len(), 2);
    let caller = &lowered.semantic_module.machines[0];
    let (call_block, call) = caller
        .blocks
        .iter()
        .find_map(|block| {
            block
                .operations
                .iter()
                .find(|operation| matches!(operation.kind, OperationKind::Call { .. }))
                .map(|operation| (block, operation))
        })
        .expect("the staged convergence block should contain one terminal call");
    let OperationKind::Call { arguments, .. } = &call.kind else {
        unreachable!()
    };
    assert_eq!(arguments.len(), 2);
    assert!(arguments.iter().all(|argument| {
        call_block
            .parameters
            .iter()
            .any(|parameter| parameter.id == *argument)
    }));
    assert!(caller.blocks.iter().any(|block| matches!(
        block.terminator,
        terminal_psi::Terminator::Conditional { .. }
    )));

    let verified = verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the staged source-call closure should verify");
    for (first, second) in [(false, false), (false, true), (true, false), (true, true)] {
        assert_eq!(
            interpret_verified_artifact(
                &verified,
                &[
                    TerminalScalarValue::Boolean(first),
                    TerminalScalarValue::Boolean(second),
                ],
            )
            .expect("the staged source call should interpret")
            .value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(first || second)),
        );
    }
    let abstract_operations = lower_verified_artifact(&verified)
        .expect("the staged source call should reach Omega lowering");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("the staged source call should select a native calling plan");
        let assigned =
            assign_registers(&target_operations).expect("the staged source call should assign");
        let emitted = emit_machine_code(&assigned).expect("the staged source call should emit");
        assert_eq!(emitted.functions.len(), 2);
        assert!(
            emitted.functions[0].internal_calls.len() > 1,
            "the convergence operation should be source-distributed"
        );
        assert!(
            emitted.functions[0]
                .internal_calls
                .iter()
                .all(|call| call.owner == emitted.functions[0].internal_calls[0].owner),
            "every distributed call retains the one convergence operation owner"
        );
        assert!(emitted.functions[0].scalar_stack.is_some());
        assert!(
            emitted.functions[0]
                .internal_calls
                .iter()
                .all(
                    |call| call.target == MachineId::new(2).expect("callee identity")
                        && call.scalar_stack.is_some()
                )
        );
        let object = build_object_artifact(&emitted)
            .expect("the source-distributed convergence tree should replay at object boundary");
        let demand = derive_stack_demand(&object, MachineId::new(1).unwrap())
            .expect("the convergence tree should compose its staged call closure");
        assert_eq!(demand.contributing_machines().len(), 2);
    }
}

#[test]
fn checked_source_guarded_short_circuit_call_argument_uses_the_staged_value() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("the guarded short-circuit call canary should compile");
    let lowered = lower_machine(&checked, "terminal_call_short_guarded")
        .expect("callee-relative crash routes should bind the staged terminal argument");
    let call = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        .expect("the staged guarded caller should contain one terminal call");
    let OperationKind::Call {
        arguments,
        crash_continuations,
        ..
    } = &call.kind
    else {
        unreachable!()
    };
    assert_eq!(arguments.len(), 3);
    let [continuation] = crash_continuations.as_slice() else {
        panic!("the staged invocation should retain one guarded crash bucket")
    };
    let [terminal_psi::CrashRouteGuard::Predicate(predicate)] =
        continuation.alternatives.as_slice()
    else {
        panic!("the staged crash route should remain conditional")
    };
    let predicate_value = match predicate.proposition() {
        semantic_vocabulary::Proposition::Equal(left, right) => [left, right]
            .into_iter()
            .find_map(|term| match term {
                semantic_vocabulary::ScalarTerm::BooleanNot { operand } => match operand.as_ref() {
                    semantic_vocabulary::ScalarTerm::Value {
                        id,
                        scalar_type: ScalarType::Boolean,
                    } => Some(*id),
                    _ => None,
                },
                _ => None,
            })
            .expect("the staged continuation should negate its first call argument"),
        other => panic!("unexpected staged guarded continuation term: {other:?}"),
    };
    assert_eq!(
        predicate_value, arguments[2],
        "the callee parameter must bind the exact staged terminal value",
    );
    assert_ne!(
        predicate_value, arguments[1],
        "an overlapping argument expression must not erase callee parameter position",
    );

    let verified = verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the guarded staged source-call closure should verify");
    let mut execution = start_verified_artifact(
        &verified,
        &[
            TerminalScalarValue::Boolean(false),
            TerminalScalarValue::Boolean(true),
        ],
    )
    .expect("start the true staged guarded invocation");
    assert!(matches!(
        execution
            .resume(&mut TerminalFuelMeter::unbounded())
            .expect("the staged callee crash should execute"),
        TerminalExecutionStatus::Crashed(terminal_interpreter::TerminalCrash {
            cause: CrashCause::Trap,
            ..
        })
    ));
    assert_eq!(
        interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(true),
                TerminalScalarValue::Boolean(true),
            ],
        )
        .expect("a false staged guard should return")
        .value(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(false)),
    );
}

#[test]
fn aggregate_member_crash_contract_fails_closed_at_terminal_production() {
    let canary = terminal_source_canary(fixture_roster::MEMBER_CRASH_CONTRACT_BOUNDARY);
    let checked = compile_to_checked(&canary, None).unwrap_or_else(|diagnostics| {
        panic!(
            "aggregate/member crash-contract boundary should reach checked semantics:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });

    let scalar = lower_machine(&checked, "scalar_guarded")
        .expect("the paired scalar crash contract should lower to terminal Psi");
    verify_module(
        &scalar.semantic_module,
        &scalar.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the paired scalar crash contract should verify from its artifact");

    assert_eq!(
        lower_machine(&checked, "member_guarded")
            .expect_err("aggregate/member crash predicates must remain fail-closed"),
        LoweringError::Unsupported("machine has no source-independent checked scalar control plan")
    );
}

#[test]
fn checked_source_guarded_call_uses_invocation_specific_crash_terms() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("the guarded-call source canary should compile");
    let lowered = lower_machine(&checked, "terminal_call_guarded_caller")
        .expect("checked guarded calls should compose a terminal machine closure");
    let call = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        .expect("the caller should contain one terminal call");
    let OperationKind::Call {
        arguments,
        crash_continuations,
        ..
    } = &call.kind
    else {
        unreachable!()
    };
    let [continuation] = crash_continuations.as_slice() else {
        panic!("the invocation should retain one guarded crash bucket")
    };
    let [terminal_psi::CrashRouteGuard::Predicate(predicate)] =
        continuation.alternatives.as_slice()
    else {
        panic!("the invocation crash route should remain guarded")
    };
    let predicate_value = match predicate.proposition() {
        semantic_vocabulary::Proposition::Equal(left, right) => [left, right]
            .into_iter()
            .find_map(|term| match term {
                semantic_vocabulary::ScalarTerm::Value {
                    id,
                    scalar_type: ScalarType::Boolean,
                } => Some(*id),
                _ => None,
            })
            .expect("the guarded continuation should refer to the caller-local value"),
        other => panic!("unexpected guarded continuation term: {other:?}"),
    };
    assert_eq!(
        predicate_value, arguments[0],
        "the checked computed local should lower to the call argument's terminal ValueId",
    );
    assert_ne!(
        predicate_value, lowered.semantic_module.machines[0].parameters[0].id,
        "the computed local must not collapse to the caller parameter",
    );

    let verified = verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the guarded source-call closure should verify");
    assert_eq!(
        interpret_verified_artifact(&verified, &[TerminalScalarValue::Boolean(true)])
            .expect("the disproved call route should return")
            .value(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(false)),
    );
    let mut execution = start_verified_artifact(&verified, &[TerminalScalarValue::Boolean(false)])
        .expect("start the proved guarded-crash invocation");
    assert!(matches!(
        execution
            .resume(&mut TerminalFuelMeter::unbounded())
            .expect("the callee crash should execute"),
        TerminalExecutionStatus::Crashed(terminal_interpreter::TerminalCrash {
            cause: CrashCause::Trap,
            ..
        })
    ));
    let abstract_operations = lower_verified_artifact(&verified)
        .expect("the guarded source call should reach Omega lowering");
    assert!(matches!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .find(|operation| matches!(operation, AbstractOperation::Call { .. })),
        Some(AbstractOperation::Call { .. })
    ));
    assert!(
        abstract_operations.functions[1]
            .operations
            .iter()
            .any(|operation| matches!(operation, AbstractOperation::Crash { .. }))
    );
}

#[test]
fn checked_source_direct_return_short_circuit_local_uses_terminal_control() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("short-circuit Boolean-local source canary should compile");
    let lowered = lower_machine(&checked, "terminal_short_circuit_boolean_local")
        .expect("a directly returned short-circuit local should lower as terminal control");
    drop(checked);

    let semantic = encode_module(&lowered.semantic_module)
        .expect("short-circuit-local terminal Psi should encode");
    let proof = encode_proof_bundle(&lowered.proof_bundle)
        .expect("short-circuit-local proof should encode");
    let semantic =
        decode_module(&semantic).expect("short-circuit-local terminal Psi should decode");
    let proof = decode_proof_bundle(&proof).expect("short-circuit-local proof should decode");
    let verified = verify_module(&semantic, &proof, &AdmissionProfile::default())
        .expect("short-circuit-local terminal Psi should verify");
    assert!(semantic.machines[0].blocks.len() > 1);

    for (first, second, expected) in [
        (false, false, false),
        (false, true, false),
        (true, false, false),
        (true, true, true),
    ] {
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
            ],
        )
        .expect("short-circuit local should execute");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("verified short-circuit local should lower without frontend state");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("short-circuit local should lower for both native targets");
        let assigned =
            assign_registers(&target_operations).expect("short-circuit local should assign");
        let emitted = emit_machine_code(&assigned).expect("short-circuit local should emit");
        assert!(!emitted.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_strict_short_circuit_local_use_preserves_terminal_control() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("consumed short-circuit Boolean-local source canary should compile");
    let lowered = lower_machine(&checked, "terminal_consumed_short_circuit_boolean_local")
        .expect("a strictly consumed short-circuit local should lower as terminal control");
    drop(checked);

    let semantic = encode_module(&lowered.semantic_module)
        .expect("consumed short-circuit-local terminal Psi should encode");
    let proof = encode_proof_bundle(&lowered.proof_bundle)
        .expect("consumed short-circuit-local proof should encode");
    let semantic =
        decode_module(&semantic).expect("consumed short-circuit-local terminal Psi should decode");
    let proof =
        decode_proof_bundle(&proof).expect("consumed short-circuit-local proof should decode");
    let verified = verify_module(&semantic, &proof, &AdmissionProfile::default())
        .expect("consumed short-circuit-local terminal Psi should verify");
    assert!(semantic.machines[0].blocks.len() > 1);

    for (first, second, expected) in [
        (false, false, true),
        (false, true, true),
        (true, false, true),
        (true, true, false),
    ] {
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
            ],
        )
        .expect("consumed short-circuit local should execute");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("verified consumed short-circuit local should lower without frontend state");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("consumed short-circuit local should lower for both native targets");
        let assigned = assign_registers(&target_operations)
            .expect("consumed short-circuit local should assign");
        let emitted =
            emit_machine_code(&assigned).expect("consumed short-circuit local should emit");
        assert!(!emitted.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_reused_short_circuit_local_is_carried_once() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("reused short-circuit Boolean-local source canary should compile");
    let lowered = lower_machine(&checked, "terminal_reused_short_circuit_boolean_local")
        .expect("a reused short-circuit local should lower through a carried block value");
    drop(checked);

    let machine = &lowered.semantic_module.machines[0];
    assert!(machine.blocks.iter().any(|block| {
        block.parameters.len() == 3
            && block
                .parameters
                .last()
                .is_some_and(|parameter| parameter.scalar_type == ScalarType::Boolean)
    }));
    assert_eq!(
        machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(operation.kind, OperationKind::BooleanEqual { .. }))
            .count(),
        1
    );

    let semantic = encode_module(&lowered.semantic_module)
        .expect("reused short-circuit-local terminal Psi should encode");
    let proof = encode_proof_bundle(&lowered.proof_bundle)
        .expect("reused short-circuit-local proof should encode");
    let semantic =
        decode_module(&semantic).expect("reused short-circuit-local terminal Psi should decode");
    let proof =
        decode_proof_bundle(&proof).expect("reused short-circuit-local proof should decode");
    let verified = verify_module(&semantic, &proof, &AdmissionProfile::default())
        .expect("reused short-circuit-local terminal Psi should verify");

    for (first, second, expected_units) in [
        (false, false, 5_u64),
        (false, true, 5),
        (true, false, 6),
        (true, true, 6),
    ] {
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
            ],
        )
        .expect("reused short-circuit local should execute");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
        );
        assert_eq!(measured.usage().total_units(), expected_units);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("verified reused short-circuit local should lower without frontend state");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("reused short-circuit local should lower for both native targets");
        let assigned =
            assign_registers(&target_operations).expect("reused short-circuit local should assign");
        let emitted = emit_machine_code(&assigned).expect("reused short-circuit local should emit");
        assert!(!emitted.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_short_circuit_local_is_carried_into_a_branch_guard() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("branched short-circuit Boolean-local source canary should compile");
    let lowered = lower_machine(&checked, "terminal_branched_short_circuit_boolean_local")
        .expect("a short-circuit local should be carried into terminal branch control");
    drop(checked);

    let machine = &lowered.semantic_module.machines[0];
    assert!(machine.blocks.iter().any(|block| {
        block.parameters.len() == 3
            && block
                .parameters
                .last()
                .is_some_and(|parameter| parameter.scalar_type == ScalarType::Boolean)
            && matches!(block.terminator, Terminator::Conditional { .. })
    }));

    let semantic = encode_module(&lowered.semantic_module)
        .expect("branched short-circuit-local terminal Psi should encode");
    let proof = encode_proof_bundle(&lowered.proof_bundle)
        .expect("branched short-circuit-local proof should encode");
    let semantic =
        decode_module(&semantic).expect("branched short-circuit-local terminal Psi should decode");
    let proof =
        decode_proof_bundle(&proof).expect("branched short-circuit-local proof should decode");
    let verified = verify_module(&semantic, &proof, &AdmissionProfile::default())
        .expect("branched short-circuit-local terminal Psi should verify");

    for (first, second, expected, expected_units) in [
        (false, false, false, 6_u64),
        (false, true, false, 6),
        (true, false, false, 7),
        (true, true, true, 7),
    ] {
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
            ],
        )
        .expect("branched short-circuit local should execute");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
        assert_eq!(measured.usage().total_units(), expected_units);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("verified branched short-circuit local should lower without frontend state");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("branched short-circuit local should lower for both native targets");
        let assigned = assign_registers(&target_operations)
            .expect("branched short-circuit local should assign");
        let emitted =
            emit_machine_code(&assigned).expect("branched short-circuit local should emit");
        assert!(!emitted.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_multiple_short_circuit_locals_are_staged_left_to_right() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("multiple short-circuit Boolean-local source canary should compile");
    let lowered = lower_machine(&checked, "terminal_two_short_circuit_boolean_locals")
        .expect("multiple short-circuit locals should stage through terminal block values");
    drop(checked);

    let machine = &lowered.semantic_module.machines[0];
    assert!(machine.blocks.iter().any(|block| {
        block.parameters.len() == 4
            && block.parameters[2..]
                .iter()
                .all(|parameter| parameter.scalar_type == ScalarType::Boolean)
    }));
    assert_eq!(
        machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(operation.kind, OperationKind::BooleanEqual { .. }))
            .count(),
        1
    );

    let semantic = encode_module(&lowered.semantic_module)
        .expect("multiple short-circuit-local terminal Psi should encode");
    let proof = encode_proof_bundle(&lowered.proof_bundle)
        .expect("multiple short-circuit-local proof should encode");
    let semantic =
        decode_module(&semantic).expect("multiple short-circuit-local terminal Psi should decode");
    let proof =
        decode_proof_bundle(&proof).expect("multiple short-circuit-local proof should decode");
    let verified = verify_module(&semantic, &proof, &AdmissionProfile::default())
        .expect("multiple short-circuit-local terminal Psi should verify");

    for (first, second, expected) in [
        (false, false, true),
        (false, true, false),
        (true, false, false),
        (true, true, true),
    ] {
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
            ],
        )
        .expect("multiple short-circuit locals should execute");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
        assert_eq!(measured.usage().total_units(), 9);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("verified multiple short-circuit locals should lower without frontend state");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("multiple short-circuit locals should lower for both native targets");
        let assigned = assign_registers(&target_operations)
            .expect("multiple short-circuit locals should assign");
        let emitted =
            emit_machine_code(&assigned).expect("multiple short-circuit locals should emit");
        assert!(!emitted.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_staged_local_composes_with_a_short_circuit_return() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("staged-local short-circuit-return source canary should compile");
    let lowered = lower_machine(
        &checked,
        "terminal_short_circuit_local_then_short_circuit_return",
    )
    .expect("a staged local should compose with short-circuit terminal return control");
    drop(checked);

    let semantic = encode_module(&lowered.semantic_module)
        .expect("staged-local short-circuit-return terminal Psi should encode");
    let proof = encode_proof_bundle(&lowered.proof_bundle)
        .expect("staged-local short-circuit-return proof should encode");
    let semantic = decode_module(&semantic)
        .expect("staged-local short-circuit-return terminal Psi should decode");
    let proof =
        decode_proof_bundle(&proof).expect("staged-local short-circuit-return proof should decode");
    let verified = verify_module(&semantic, &proof, &AdmissionProfile::default())
        .expect("staged-local short-circuit-return terminal Psi should verify");

    for (first, second, expected, expected_units) in [
        (false, false, false, 7_u64),
        (false, true, false, 7),
        (true, false, true, 8),
        (true, true, true, 7),
    ] {
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
            ],
        )
        .expect("staged-local short-circuit return should execute");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
        assert_eq!(measured.usage().total_units(), expected_units);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("verified staged-local short-circuit return should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("staged-local short-circuit return should lower for both native targets");
        let assigned = assign_registers(&target_operations)
            .expect("staged-local short-circuit return should assign");
        let emitted =
            emit_machine_code(&assigned).expect("staged-local short-circuit return should emit");
        assert!(!emitted.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_staged_local_is_carried_through_a_jump_argument() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("staged-local jump source canary should compile");
    let lowered = lower_machine(&checked, "terminal_short_circuit_local_jump")
        .expect("a staged local should cross an ordinary terminal jump argument");
    drop(checked);

    let semantic = encode_module(&lowered.semantic_module)
        .expect("staged-local jump terminal Psi should encode");
    let proof =
        encode_proof_bundle(&lowered.proof_bundle).expect("staged-local jump proof should encode");
    let semantic = decode_module(&semantic).expect("staged-local jump terminal Psi should decode");
    let proof = decode_proof_bundle(&proof).expect("staged-local jump proof should decode");
    let verified = verify_module(&semantic, &proof, &AdmissionProfile::default())
        .expect("staged-local jump terminal Psi should verify");

    for (first, second, expected, expected_units) in [
        (false, false, false, 5_u64),
        (false, true, false, 5),
        (true, false, false, 6),
        (true, true, true, 6),
    ] {
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
            ],
        )
        .expect("staged-local jump should execute");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
        assert_eq!(measured.usage().total_units(), expected_units);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("verified staged-local jump should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("staged-local jump should lower for both native targets");
        let assigned =
            assign_registers(&target_operations).expect("staged-local jump should assign");
        let emitted = emit_machine_code(&assigned).expect("staged-local jump should emit");
        assert!(!emitted.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_staged_local_composes_with_a_short_circuit_jump_argument() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("staged-local nested-jump source canary should compile");
    let lowered = lower_machine(&checked, "terminal_short_circuit_local_nested_jump")
        .expect("a staged local should compose with a short-circuit terminal jump argument");
    drop(checked);

    let semantic = encode_module(&lowered.semantic_module)
        .expect("staged-local nested-jump terminal Psi should encode");
    let proof = encode_proof_bundle(&lowered.proof_bundle)
        .expect("staged-local nested-jump proof should encode");
    let semantic =
        decode_module(&semantic).expect("staged-local nested-jump terminal Psi should decode");
    let proof = decode_proof_bundle(&proof).expect("staged-local nested-jump proof should decode");
    let verified = verify_module(&semantic, &proof, &AdmissionProfile::default())
        .expect("staged-local nested-jump terminal Psi should verify");

    for (first, second, expected, expected_units) in [
        (false, false, false, 8_u64),
        (false, true, false, 8),
        (true, false, true, 9),
        (true, true, true, 8),
    ] {
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
            ],
        )
        .expect("staged-local nested jump should execute");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
        assert_eq!(measured.usage().total_units(), expected_units);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("verified staged-local nested jump should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("staged-local nested jump should lower for both native targets");
        let assigned =
            assign_registers(&target_operations).expect("staged-local nested jump should assign");
        let emitted = emit_machine_code(&assigned).expect("staged-local nested jump should emit");
        assert!(!emitted.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_staged_local_composes_with_short_circuit_jump_tuple() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("staged-local nested-jump tuple source canary should compile");
    let lowered = lower_machine(&checked, "terminal_short_circuit_local_nested_jump_tuple")
        .expect("a staged local should compose with left-to-right jump-argument staging");
    drop(checked);

    let semantic = encode_module(&lowered.semantic_module)
        .expect("staged-local jump-tuple terminal Psi should encode");
    let proof = encode_proof_bundle(&lowered.proof_bundle)
        .expect("staged-local jump-tuple proof should encode");
    let semantic =
        decode_module(&semantic).expect("staged-local jump-tuple terminal Psi should decode");
    let proof = decode_proof_bundle(&proof).expect("staged-local jump-tuple proof should decode");
    let verified = verify_module(&semantic, &proof, &AdmissionProfile::default())
        .expect("staged-local jump-tuple terminal Psi should verify");

    for (first, second, expected, expected_units) in [
        (false, false, true, 14_u64),
        (false, true, true, 14),
        (true, false, false, 15),
        (true, true, true, 15),
    ] {
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
            ],
        )
        .expect("staged-local jump tuple should execute");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
        assert_eq!(measured.usage().total_units(), expected_units);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("verified staged-local jump tuple should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("staged-local jump tuple should lower for both native targets");
        let assigned =
            assign_registers(&target_operations).expect("staged-local jump tuple should assign");
        let emitted = emit_machine_code(&assigned).expect("staged-local jump tuple should emit");
        assert!(!emitted.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_staged_local_keeps_short_circuit_edge_arguments_arm_local() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("staged-local conditional-edge source canary should compile");
    let lowered = lower_machine(&checked, "terminal_short_circuit_local_conditional_edge")
        .expect("staged locals should compose with selected conditional-edge staging");
    drop(checked);

    let semantic = encode_module(&lowered.semantic_module)
        .expect("staged-local conditional-edge terminal Psi should encode");
    let proof = encode_proof_bundle(&lowered.proof_bundle)
        .expect("staged-local conditional-edge proof should encode");
    let semantic =
        decode_module(&semantic).expect("staged-local conditional-edge terminal Psi should decode");
    let proof =
        decode_proof_bundle(&proof).expect("staged-local conditional-edge proof should decode");
    let verified = verify_module(&semantic, &proof, &AdmissionProfile::default())
        .expect("staged-local conditional-edge terminal Psi should verify");

    for (first, second, expected, expected_units) in [
        (false, false, false, 9_u64),
        (false, true, false, 9),
        (true, false, true, 11),
        (true, true, true, 10),
    ] {
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
            ],
        )
        .expect("staged-local conditional edge should execute");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
        assert_eq!(measured.usage().total_units(), expected_units);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("verified staged-local conditional edge should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("staged-local conditional edge should lower for both native targets");
        let assigned = assign_registers(&target_operations)
            .expect("staged-local conditional edge should assign");
        let emitted =
            emit_machine_code(&assigned).expect("staged-local conditional edge should emit");
        assert!(!emitted.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_staged_local_composes_with_a_short_circuit_guard() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("staged-local short-circuit-guard source canary should compile");
    let lowered = lower_machine(&checked, "terminal_short_circuit_local_guard")
        .expect("a staged local should compose with short-circuit terminal guard control");
    drop(checked);

    let semantic = encode_module(&lowered.semantic_module)
        .expect("staged-local short-circuit-guard terminal Psi should encode");
    let proof = encode_proof_bundle(&lowered.proof_bundle)
        .expect("staged-local short-circuit-guard proof should encode");
    let semantic = decode_module(&semantic)
        .expect("staged-local short-circuit-guard terminal Psi should decode");
    let proof =
        decode_proof_bundle(&proof).expect("staged-local short-circuit-guard proof should decode");
    let verified = verify_module(&semantic, &proof, &AdmissionProfile::default())
        .expect("staged-local short-circuit-guard terminal Psi should verify");

    for (first, second, expected, expected_units) in [
        (false, false, false, 8_u64),
        (false, true, false, 8),
        (true, false, true, 9),
        (true, true, true, 8),
    ] {
        let measured = interpret_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
            ],
        )
        .expect("staged-local short-circuit guard should execute");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
        assert_eq!(measured.usage().total_units(), expected_units);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("verified staged-local short-circuit guard should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("staged-local short-circuit guard should lower for both native targets");
        let assigned = assign_registers(&target_operations)
            .expect("staged-local short-circuit guard should assign");
        let emitted =
            emit_machine_code(&assigned).expect("staged-local short-circuit guard should emit");
        assert!(!emitted.functions[0].bytes.is_empty());
    }
}

#[test]
fn checked_source_staged_local_sequences_before_an_explicit_crash() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("staged-local crash source canary should compile");
    let lowered = lower_machine(&checked, "terminal_short_circuit_local_crash")
        .expect("a staged local should sequence before terminal crash control");
    drop(checked);

    let semantic = encode_module(&lowered.semantic_module)
        .expect("staged-local crash terminal Psi should encode");
    let proof =
        encode_proof_bundle(&lowered.proof_bundle).expect("staged-local crash proof should encode");
    let semantic = decode_module(&semantic).expect("staged-local crash terminal Psi should decode");
    let proof = decode_proof_bundle(&proof).expect("staged-local crash proof should decode");
    let verified = verify_module(&semantic, &proof, &AdmissionProfile::default())
        .expect("staged-local crash terminal Psi should verify");

    for (first, second, expected_units) in [
        (false, false, 4_u64),
        (false, true, 4),
        (true, false, 5),
        (true, true, 5),
    ] {
        let mut execution = start_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(first),
                TerminalScalarValue::Boolean(second),
            ],
        )
        .expect("staged-local crash execution should start");
        let mut meter = TerminalFuelMeter::unbounded();
        assert!(matches!(
            execution
                .resume(&mut meter)
                .expect("staged-local crash should execute"),
            TerminalExecutionStatus::Crashed(terminal_interpreter::TerminalCrash {
                cause: CrashCause::Abort,
                ..
            })
        ));
        assert_eq!(meter.usage().total_units(), expected_units);
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("verified staged-local crash should cross the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| matches!(operation, AbstractOperation::Crash { .. }))
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("staged-local crash should lower for both native targets");
        let assigned =
            assign_registers(&target_operations).expect("staged-local crash should assign");
        let emitted = emit_machine_code(&assigned).expect("staged-local crash should emit");
        assert!(!emitted.functions[0].bytes.is_empty());
    }
}
