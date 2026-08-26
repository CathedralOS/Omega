use super::*;

#[test]
fn psi_terminal_producer_rejects_source_outside_its_declared_slice() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi source canary should compile");
    let attached_unit = lower_machine(&checked, "Main::main")
        .expect("empty attached Unit machines are in the structural terminal-Psi slice");
    assert!(
        attached_unit.semantic_module.machines[0]
            .attachment
            .is_some()
    );
    verify_module(
        &attached_unit.semantic_module,
        &attached_unit.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the attached Unit artifact should verify without producer state");
    assert_eq!(
        lower_machine(&checked, "terminal_closed_integer_chain_wrong_contract")
            .expect_err("closed chain with an unrelated contract must fail closed"),
        LoweringError::Unsupported("contract literals must equal the executed literal")
    );
    assert_eq!(
        lower_machine(&checked, "terminal_known_integer_graph_wrong_contract")
            .expect_err("compile-known integer graph with an unrelated contract must fail closed"),
        LoweringError::Unsupported("contract literals must equal the executed literal")
    );
    assert_eq!(
        lower_machine(&checked, "terminal_known_boolean_binding_wrong_contract").expect_err(
            "compile-known Boolean binding with an unrelated integer contract must fail closed"
        ),
        LoweringError::Unsupported("contract literals must equal the executed literal")
    );
    assert_eq!(
        lower_machine(&checked, "terminal_boolean_chain_wrong_contract")
            .expect_err("closed Boolean chain with an unrelated contract must fail closed"),
        LoweringError::Unsupported("Boolean contract literal must match the compile-known result")
    );
    assert_eq!(
        lower_machine(&checked, "terminal_boolean_tuple_wrong_contract")
            .expect_err("compile-known general graph with an unrelated contract must fail closed"),
        LoweringError::Unsupported("Boolean contract literal must match the compile-known result")
    );
    assert_eq!(
        lower_machine(&checked, "terminal_unpublished_abort")
            .expect_err("an unpublished crash cannot enter the terminal-Psi source slice"),
        LoweringError::Unsupported(
            "an explicit crash in the terminal-Psi source slice requires exactly one prechecked covering route bucket"
        )
    );

    let mut missing_site = checked.clone();
    let terminal_abort = missing_site
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "terminal_abort")
        .expect("terminal abort machine")
        .symbol;
    let crash = &mut missing_site
        .facts
        .contract_plans
        .machines
        .iter_mut()
        .find(|plan| plan.machine == terminal_abort)
        .expect("terminal abort contract plan")
        .crash;
    *crash = psi_checked_trees::CrashPlan::published_ceiling(crash.published().to_vec());
    assert_eq!(
        lower_machine(&missing_site, "terminal_abort")
            .expect_err("terminal production must consume checked crash-site evidence"),
        LoweringError::Unsupported("explicit crash has no body-derived checked crash-site row")
    );

    let mut missing_coverage = checked.clone();
    let crash = &mut missing_coverage
        .facts
        .contract_plans
        .machines
        .iter_mut()
        .find(|plan| plan.machine == terminal_abort)
        .expect("terminal abort contract plan")
        .crash;
    let site = crash
        .checked_sites()
        .first()
        .expect("terminal abort checked site");
    let uncovered_site = psi_checked_trees::CheckedCrashSite::new(
        site.location(),
        site.cause(),
        Vec::new(),
        site.frontier_lower_bound().to_vec(),
    );
    *crash = psi_checked_trees::CrashPlan::published_ceiling(crash.published().to_vec())
        .with_checked_sites(vec![uncovered_site])
        .expect("uncovered site still has a valid checked location");
    assert_eq!(
        lower_machine(&missing_coverage, "terminal_abort")
            .expect_err("terminal production must consume checked guard coverage"),
        LoweringError::Unsupported(
            "an explicit crash in the terminal-Psi source slice requires exactly one prechecked covering route bucket"
        )
    );

    let mut unmapped_frontier = checked.clone();
    let crash = &mut unmapped_frontier
        .facts
        .contract_plans
        .machines
        .iter_mut()
        .find(|plan| plan.machine == terminal_abort)
        .expect("terminal abort contract plan")
        .crash;
    let site = crash
        .checked_sites()
        .first()
        .expect("terminal abort checked site");
    let claim = psi_language_semantics::PermissionClaimIdentity::Established {
        machine_symbol: terminal_abort,
        state_symbol: site.location().state(),
        source: psi_language_semantics::PermissionEventSource::StateEntry,
        ordinal: 0,
    };
    let site_with_frontier = psi_checked_trees::CheckedCrashSite::new(
        site.location(),
        site.cause(),
        site.guard_covering_buckets().to_vec(),
        vec![claim],
    );
    *crash = crash
        .clone()
        .with_checked_sites(vec![site_with_frontier])
        .expect("known claim identity is valid checked crash evidence");
    assert_eq!(
        lower_machine(&unmapped_frontier, "terminal_abort")
            .expect_err("terminal production must map every checked crash-frontier claim"),
        LoweringError::CrashFrontierClaimNotLowered(claim)
    );
}

#[test]
fn boolean_result_graph_retains_guarded_crash_exit() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("Boolean guarded-crash source canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_guarded_trap")
        .expect("Boolean-result graph should retain its guarded crash exit");
    drop(checked);

    let semantic_bytes =
        encode_module(&lowered.semantic_module).expect("Boolean guarded crash should encode");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("Boolean guarded-crash proof should encode");
    let semantic_module =
        decode_module(&semantic_bytes).expect("Boolean guarded crash should decode");
    let proof_bundle =
        decode_proof_bundle(&proof_bytes).expect("Boolean guarded-crash proof should decode");
    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("Boolean guarded crash should verify after frontend drop");
    assert!(matches!(
        semantic_module.machines[0].blocks[1].terminator,
        Terminator::Crash {
            cause: CrashCause::Trap,
            ..
        }
    ));

    for (flag, expected) in [
        (true, expected_crash(&semantic_module)),
        (
            false,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                TerminalScalarValue::Boolean(true),
            )),
        ),
    ] {
        let mut execution =
            start_verified_artifact(&verified, &[TerminalScalarValue::Boolean(flag)])
                .expect("Boolean guarded-crash execution should start");
        assert_eq!(
            execution
                .resume(&mut TerminalFuelMeter::unbounded())
                .expect("Boolean guarded-crash execution should finish"),
            expected
        );
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("guarded crash should remain represented at the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| matches!(operation, TerminalAbstractOperation::Crash { .. }))
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("guarded Boolean crash should select as mixed terminal control");
        let TerminalTargetOperation::ReturnBooleanConditionalControl {
            when_true,
            when_false,
            ..
        } = &target_operations.functions[0].operation
        else {
            panic!("direct Boolean guard should retain target conditional control");
        };
        assert!(matches!(
            when_true.control.as_ref(),
            TerminalTargetBooleanControl::Crash {
                cause: CrashCause::Trap,
                site_guard,
                frontier_lower_bound,
                ..
            } if !site_guard.is_empty()
                && frontier_lower_bound.is_empty()
        ));
        assert!(matches!(
            when_false.control.as_ref(),
            TerminalTargetBooleanControl::ReturnImmediate { value: true, .. }
        ));

        let assigned = assign_registers(&target_operations)
            .expect("guarded Boolean crash control should assign");
        let TerminalAssignedOperation::ReturnBooleanConditionalControl { when_true, .. } =
            &assigned.functions[0].operation
        else {
            panic!("assigned Boolean control should retain its shape");
        };
        assert!(matches!(
            when_true.control.as_ref(),
            TerminalAssignedBooleanControl::Crash {
                cause: CrashCause::Trap,
                ..
            }
        ));
        let emitted = emit_machine_code(&assigned).expect("guarded Boolean crash should emit");
        let branch_to_false_over_fault = match target.architecture {
            omega_target::Architecture::X86_64 => {
                &[0x0f, 0x84, 0x02, 0x00, 0x00, 0x00, 0x0f, 0x0b][..]
            }
            omega_target::Architecture::Aarch64 => {
                &[0x40, 0x00, 0x00, 0x34, 0x00, 0x00, 0x20, 0xd4][..]
            }
        };
        assert!(
            emitted.functions[0]
                .bytes
                .windows(branch_to_false_over_fault.len())
                .any(|window| window == branch_to_false_over_fault),
            "the false return arm must branch over the true crash leaf"
        );
    }
}

#[test]
fn native_lowering_preserves_every_reachable_crash_leaf() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("two-leaf crash source canary should compile");
    let lowered = lower_machine(&checked, "terminal_two_crash_leaves")
        .expect("both guarded crash leaves should lower to terminal Psi");
    let verified = verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("both guarded crash leaves should verify");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");

    for (trap, abort, expected_crash) in [
        (true, false, Some(CrashCause::Trap)),
        (false, true, Some(CrashCause::Abort)),
        (true, true, Some(CrashCause::Trap)),
        (false, false, None),
    ] {
        let mut execution = start_verified_artifact(
            &verified,
            &[
                TerminalScalarValue::Boolean(trap),
                TerminalScalarValue::Boolean(abort),
            ],
        )
        .expect("two-leaf crash execution should start");
        let status = execution
            .resume(&mut TerminalFuelMeter::unbounded())
            .expect("two-leaf crash execution should finish");
        match (status, expected_crash) {
            (TerminalExecutionStatus::Crashed(crash), Some(cause)) => {
                assert_eq!(crash.cause, cause);
            }
            (
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                    TerminalScalarValue::Integer {
                        scalar_type,
                        value: IntegerValue::Signed(0),
                    },
                )),
                None,
            ) => assert_eq!(scalar_type, i32_type),
            (status, expected) => {
                panic!("unexpected two-leaf outcome {status:?}; expected crash cause {expected:?}")
            }
        }
    }

    let abstract_operations = lower_verified_artifact(&verified)
        .expect("two crash leaves should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("two crash leaves should survive target selection");
        let function = &target_operations.functions[0];
        let leaves = target_integer_crash_leaves(&function.operation);
        assert_eq!(leaves.len(), 2);
        assert!(leaves.iter().any(|(_, cause)| *cause == CrashCause::Trap));
        assert!(leaves.iter().any(|(_, cause)| *cause == CrashCause::Abort));
        for (edge, _) in &leaves {
            assert!(function.provenance.edges.contains(edge));
        }

        let assigned = assign_registers(&target_operations)
            .expect("two-leaf conditional control should assign");
        let emitted = emit_machine_code(&assigned).expect("two crash leaves should emit");
        let emitted_function = &emitted.functions[0];
        for (edge, _) in &leaves {
            assert!(emitted_function.provenance.edges.contains(edge));
        }
        let fault = match target.architecture {
            omega_target::Architecture::X86_64 => &[0x0f, 0x0b][..],
            omega_target::Architecture::Aarch64 => &[0x00, 0x00, 0x20, 0xd4][..],
        };
        assert_eq!(
            emitted_function
                .bytes
                .windows(fault.len())
                .filter(|window| *window == fault)
                .count(),
            2,
            "one native fault instruction must remain for each reachable crash leaf"
        );
    }
}

#[test]
fn explicit_source_crash_lowers_to_verified_nonreturning_terminal() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi source canary should compile");
    let wide_trap = lower_machine(&checked, "terminal_wide_trap")
        .expect("an unconditional published trap route should lower");
    let [wide_route] = wide_trap.semantic_module.machines[0]
        .contract
        .crash_routes
        .as_slice()
    else {
        panic!("unconditional trap should publish exactly one crash route");
    };
    assert_eq!(wide_route.cause, CrashCause::Trap);
    assert!(matches!(
        wide_route.alternatives.as_slice(),
        [CrashRouteGuard::Truth]
    ));
    assert!(matches!(
        &wide_trap.semantic_module.machines[0].blocks[0].terminator,
        Terminator::Crash {
            cause: CrashCause::Trap,
            site_guard,
            ..
        } if site_guard.is_empty()
    ));
    let guarded_trap = lower_machine(&checked, "terminal_path_guarded_trap")
        .expect("checked incoming guard coverage should open a guarded crash branch");
    assert!(matches!(
        &guarded_trap.semantic_module.machines[0].blocks[1].terminator,
        Terminator::Crash {
            cause: CrashCause::Trap,
            site_guard,
            ..
        } if !site_guard.is_empty()
    ));
    let guarded_semantic_bytes =
        encode_module(&guarded_trap.semantic_module).expect("guarded crash should encode");
    let guarded_proof_bytes =
        encode_proof_bundle(&guarded_trap.proof_bundle).expect("guarded crash proof should encode");
    let guarded_semantic_module =
        decode_module(&guarded_semantic_bytes).expect("guarded crash should decode");
    let guarded_proof_bundle =
        decode_proof_bundle(&guarded_proof_bytes).expect("guarded crash proof should decode");
    let guarded_verified = verify_module(
        &guarded_semantic_module,
        &guarded_proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the guarded crash branch should verify");
    assert_eq!(
        derive_fixed_entry_fuel(&guarded_verified, guarded_semantic_module.entry)
            .expect("guarded crash control should have a fixed entry ceiling")
            .ceiling_units(),
        3
    );
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    for (flag, expected) in [
        (true, expected_crash(&guarded_semantic_module)),
        (
            false,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                TerminalScalarValue::Integer {
                    scalar_type: i32_type,
                    value: IntegerValue::Signed(0),
                },
            )),
        ),
    ] {
        let mut execution =
            start_verified_artifact(&guarded_verified, &[TerminalScalarValue::Boolean(flag)])
                .expect("guarded crash execution should start");
        let mut guarded_meter = TerminalFuelMeter::unbounded();
        assert_eq!(execution.resume(&mut guarded_meter).unwrap(), expected);
    }
    let guarded_abstract = lower_verified_artifact(&guarded_verified)
        .expect("guarded integer crash should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&guarded_abstract, target)
            .expect("guarded integer crash should select as mixed terminal control");
        let TerminalTargetOperation::ReturnIntegerConditionalControl {
            when_true,
            when_false,
            ..
        } = &target_operations.functions[0].operation
        else {
            panic!("direct Boolean guard should retain integer target control");
        };
        assert!(matches!(
            when_true.control.as_ref(),
            TerminalTargetIntegerControl::Crash {
                cause: CrashCause::Trap,
                site_guard,
                frontier_lower_bound,
                ..
            } if !site_guard.is_empty()
                && frontier_lower_bound.is_empty()
        ));
        assert!(matches!(
            when_false.control.as_ref(),
            TerminalTargetIntegerControl::Return { .. }
        ));

        let assigned = assign_registers(&target_operations)
            .expect("guarded integer crash control should assign");
        let TerminalAssignedOperation::ReturnIntegerConditionalControl { when_true, .. } =
            &assigned.functions[0].operation
        else {
            panic!("assigned integer control should retain its shape");
        };
        assert!(matches!(
            when_true.control.as_ref(),
            TerminalAssignedIntegerControl::Crash {
                cause: CrashCause::Trap,
                ..
            }
        ));
        let emitted = emit_machine_code(&assigned).expect("guarded integer crash should emit");
        let fault = match target.architecture {
            omega_target::Architecture::X86_64 => &[0x0f, 0x0b][..],
            omega_target::Architecture::Aarch64 => &[0x00, 0x00, 0x20, 0xd4][..],
        };
        assert!(
            emitted.functions[0]
                .bytes
                .windows(fault.len())
                .any(|window| window == fault)
        );
    }
    let integer_guarded_trap = lower_machine(&checked, "terminal_integer_guarded_trap")
        .expect("exact-type integer comparison should open a guarded crash branch");
    assert!(matches!(
        &integer_guarded_trap.semantic_module.machines[0].blocks[0].operations[..],
        [
            psi_terminal::Operation {
                kind: psi_terminal::OperationKind::IntegerConstant { .. },
                ..
            },
            psi_terminal::Operation {
                kind: psi_terminal::OperationKind::WrappingIntegerAdd { .. },
                ..
            },
            psi_terminal::Operation {
                kind: psi_terminal::OperationKind::IntegerLessOrEqual { left, right },
                ..
            },
        ] if left.get() == 2 && right.get() == 4
    ));
    let integer_guarded_semantic_bytes = encode_module(&integer_guarded_trap.semantic_module)
        .expect("integer-guarded crash should encode");
    let integer_guarded_proof_bytes = encode_proof_bundle(&integer_guarded_trap.proof_bundle)
        .expect("integer-guarded crash proof should encode");
    let integer_guarded_semantic_module = decode_module(&integer_guarded_semantic_bytes)
        .expect("integer-guarded crash should decode");
    let integer_guarded_proof_bundle = decode_proof_bundle(&integer_guarded_proof_bytes)
        .expect("integer-guarded crash proof should decode");
    let integer_guarded_verified = verify_module(
        &integer_guarded_semantic_module,
        &integer_guarded_proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the integer-guarded crash branch should verify");
    assert_eq!(
        derive_fixed_entry_fuel(
            &integer_guarded_verified,
            integer_guarded_semantic_module.entry
        )
        .expect("integer-guarded crash control should have a fixed entry ceiling")
        .ceiling_units(),
        6
    );
    for (value, limit, expected) in [
        (1, 2, expected_crash(&integer_guarded_semantic_module)),
        (
            1,
            3,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                TerminalScalarValue::Integer {
                    scalar_type: i32_type,
                    value: IntegerValue::Signed(0),
                },
            )),
        ),
    ] {
        let mut execution = start_verified_artifact(
            &integer_guarded_verified,
            &[
                TerminalScalarValue::Integer {
                    scalar_type: i32_type,
                    value: IntegerValue::Signed(value),
                },
                TerminalScalarValue::Integer {
                    scalar_type: i32_type,
                    value: IntegerValue::Signed(limit),
                },
            ],
        )
        .expect("integer-guarded crash execution should start");
        let mut meter = TerminalFuelMeter::unbounded();
        assert_eq!(execution.resume(&mut meter).unwrap(), expected);
    }
    assert_guarded_crash_emits(&integer_guarded_verified);
    let transitive_trap = lower_machine(&checked, "terminal_transitive_guarded_trap")
        .expect("a transitive integer conjunction should lower as short-circuit control");
    assert_eq!(transitive_trap.semantic_module.machines[0].blocks.len(), 4);
    assert!(matches!(
        transitive_trap.semantic_module.machines[0].blocks[0].terminator,
        Terminator::Conditional { .. }
    ));
    let transitive_semantic_bytes = encode_module(&transitive_trap.semantic_module)
        .expect("transitive guarded crash should encode");
    let transitive_proof_bytes = encode_proof_bundle(&transitive_trap.proof_bundle)
        .expect("transitive guarded crash proof should encode");
    let transitive_semantic_module =
        decode_module(&transitive_semantic_bytes).expect("transitive guarded crash should decode");
    let transitive_proof_bundle = decode_proof_bundle(&transitive_proof_bytes)
        .expect("transitive guarded crash proof should decode");
    let transitive_verified = verify_module(
        &transitive_semantic_module,
        &transitive_proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("transitive guarded crash should verify");
    assert_eq!(
        derive_fixed_entry_fuel(&transitive_verified, transitive_semantic_module.entry)
            .expect("transitive guarded crash should have fixed fuel")
            .ceiling_units(),
        6
    );
    let signed = |value| TerminalScalarValue::Integer {
        scalar_type: i32_type,
        value: IntegerValue::Signed(value),
    };
    for (left, middle, right, expected, expected_units) in [
        (
            5,
            3,
            10,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(signed(0))),
            4,
        ),
        (
            1,
            5,
            3,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(signed(0))),
            6,
        ),
        (1, 2, 3, expected_crash(&transitive_semantic_module), 5),
    ] {
        let mut execution = start_verified_artifact(
            &transitive_verified,
            &[signed(left), signed(middle), signed(right)],
        )
        .expect("transitive guarded crash execution should start");
        let mut meter = TerminalFuelMeter::unbounded();
        assert_eq!(execution.resume(&mut meter).unwrap(), expected);
        assert_eq!(meter.usage().total_units(), expected_units);
    }
    assert_guarded_crash_emits(&transitive_verified);
    let implied_trap = lower_machine(&checked, "terminal_implied_guarded_trap")
        .expect("structurally implied guard coverage should reach terminal production");
    let implied_verified = verify_module(
        &implied_trap.semantic_module,
        &implied_trap.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the structurally implied crash branch should verify");
    for (flag, crashes) in [(true, true), (false, false)] {
        let mut execution =
            start_verified_artifact(&implied_verified, &[TerminalScalarValue::Boolean(flag)])
                .expect("implied crash execution should start");
        let mut implied_meter = TerminalFuelMeter::unbounded();
        assert_eq!(
            matches!(
                execution.resume(&mut implied_meter).unwrap(),
                TerminalExecutionStatus::Crashed(_)
            ),
            crashes
        );
    }
    assert_guarded_crash_emits(&implied_verified);
    let lowered = lower_machine(&checked, "terminal_abort")
        .expect("an unconditional published crash should lower");
    let explicit_true = lower_machine(&checked, "terminal_explicit_true_abort")
        .expect("an explicit-true crash route should normalize to unconditional coverage");
    assert_eq!(
        lowered.semantic_module, explicit_true.semantic_module,
        "route-less and explicit-true crash ceilings lower identically"
    );
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("source slice should emit one machine");
    };
    let [block] = machine.blocks.as_slice() else {
        panic!("crash-only source should emit one block");
    };
    assert_eq!(
        block.terminator,
        Terminator::Crash {
            edge: EdgeId::new(1).unwrap(),
            cause: CrashCause::Abort,
            site_guard: Vec::new(),
            frontier_lower_bound: Vec::new(),
        }
    );

    let verified = verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source-produced crash terminal should verify");
    let mut execution =
        start_verified_artifact(&verified, &[]).expect("verified crash terminal should start");
    let mut meter = TerminalFuelMeter::with_allowance(1);
    let expected = expected_crash(&lowered.semantic_module);
    assert_eq!(execution.resume(&mut meter).unwrap(), expected);
    let charged = meter.usage().total_units();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        expected,
        "resuming a crashed execution reports the same terminal outcome"
    );
    assert_eq!(
        meter.usage().total_units(),
        charged,
        "resuming a crash must not replay its edge"
    );

    for (source, expected_cause) in [
        (&wide_trap, CrashCause::Trap),
        (&lowered, CrashCause::Abort),
    ] {
        let semantic =
            encode_module(&source.semantic_module).expect("crash semantics should encode");
        let proof = encode_proof_bundle(&source.proof_bundle).expect("crash proof should encode");
        let abstract_operations =
            lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
                .expect("verified unconditional crash should cross the Omega boundary");
        assert!(matches!(
            abstract_operations.functions[0].operations.as_slice(),
            [TerminalAbstractOperation::Crash {
                cause,
                site_guard,
                frontier_lower_bound,
                ..
            }] if *cause == expected_cause
                && site_guard.is_empty()
                && frontier_lower_bound.is_empty()
        ));

        for (target, expected_bytes) in [
            (NativeTarget::linux_x64(), &[0x0f, 0x0b][..]),
            (NativeTarget::linux_arm64(), &[0x00, 0x00, 0x20, 0xd4][..]),
        ] {
            let target_operations = lower_to_target_operations(&abstract_operations, target)
                .expect("unconditional crash should select");
            assert!(matches!(
                &target_operations.functions[0].operation,
                TerminalTargetOperation::Crash {
                    cause,
                    site_guard,
                    frontier_lower_bound,
                    ..
                } if *cause == expected_cause
                    && site_guard.is_empty()
                    && frontier_lower_bound.is_empty()
            ));
            let assigned = assign_registers(&target_operations)
                .expect("unconditional crash should require no register homes");
            assert!(matches!(
                &assigned.functions[0].operation,
                TerminalAssignedOperation::Crash { cause, .. } if *cause == expected_cause
            ));
            let emitted = emit_machine_code(&assigned).expect("unconditional crash should emit");
            assert_eq!(emitted.functions[0].bytes, expected_bytes);
            assert_eq!(
                emitted.functions[0].provenance.edges,
                vec![EdgeId::new(1).unwrap()]
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn interpreted_terminal_source_matches_emitted_host_machine_code() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi source canary should compile");
    let lowered = lower_machine(&checked, "terminal_constant")
        .expect("accepted source slice should lower to terminal Psi");
    drop(checked);

    let canonical_bytes = encode_module(&lowered.semantic_module)
        .expect("source-produced terminal Psi should encode canonically");
    let original_identity = terminal_psi_identity(&lowered.semantic_module)
        .expect("source-produced terminal Psi should have a semantic identity");
    let canonical_proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("source-produced proof bundle should encode canonically");
    let artifact_manifest =
        build_artifact_manifest(&lowered.semantic_module, &lowered.proof_bundle, None, None)
            .expect("source-produced terminal sections should have a manifest");
    drop(lowered);
    let semantic_module = decode_module(&canonical_bytes)
        .expect("canonical source-produced terminal Psi should decode");
    let proof_bundle = decode_proof_bundle(&canonical_proof_bytes)
        .expect("canonical source-produced proof bundle should decode");
    validate_artifact_manifest(
        &semantic_module,
        &proof_bundle,
        None,
        None,
        artifact_manifest,
    )
    .expect("decoded source-produced sections should match their manifest");
    assert_eq!(artifact_manifest.semantic(), original_identity);
    assert_eq!(
        terminal_psi_identity(&semantic_module).unwrap(),
        original_identity
    );

    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source-produced terminal Psi and its proof should verify");
    let fixed_fuel = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("straight-line source module should have a fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed_fuel)
        .expect("source-independent consumer should recompute the certificate");
    assert_eq!(fixed_fuel.terminal_psi(), original_identity);
    assert_eq!(fixed_fuel.ceiling_units(), 4);
    let mut execution = start_verified_artifact(&verified, &[])
        .expect("verified source-produced terminal Psi should start");
    let mut meter = TerminalFuelMeter::with_allowance(3);
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            site: FuelChargeSite::Edge(EdgeId::new(2).unwrap()),
            required_units: 1,
            remaining_units: 0,
        })
    );
    meter.replenish(1).unwrap();
    let interpreted = match execution.resume(&mut meter).unwrap() {
        TerminalExecutionStatus::Complete(value) => value,
        TerminalExecutionStatus::SponsorExhausted(_) => {
            panic!("one replenished unit should complete the source canary")
        }
        TerminalExecutionStatus::Crashed(_) => {
            panic!("the source canary has no crash exit")
        }
    };
    assert_eq!(meter.usage().schedule().marker(), 1);
    assert_eq!(meter.usage().total_units(), fixed_fuel.ceiling_units());
    assert_eq!(
        meter
            .usage()
            .at(FuelChargeSite::Operation(OperationId::new(1).unwrap()))
            .unwrap()
            .executions(),
        1,
        "resume must not replay source-produced operations"
    );
    assert_eq!(
        terminal_psi_identity(&semantic_module).unwrap(),
        original_identity,
        "fuel accounting must not change semantic identity"
    );
    let abstract_operations = lower_verified_artifact(&verified)
        .expect("verified terminal Psi should lower without source state");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("constant terminal requirements should select for the host");
    let assigned = assign_registers(&target_operations).expect("host target homes should assign");
    let machine_code = emit_machine_code(&assigned).expect("host machine code should emit");
    let object_artifact = build_terminal_object_artifact(&machine_code)
        .expect("source-produced machine code should form an owned object artifact");
    assert_eq!(object_artifact.terminal_psi(), original_identity);
    let terminal_stack_demand =
        derive_terminal_stack_demand(&object_artifact, object_artifact.entry())
            .expect("source-produced terminal stack closure");
    let entry = object_artifact.entry_function();
    assert_eq!(
        entry.provenance.operations,
        [
            OperationId::new(1).expect("entry constant"),
            OperationId::new(2).expect("return constant"),
        ]
    );
    assert_eq!(
        entry.provenance.edges,
        [
            EdgeId::new(1).expect("jump edge"),
            EdgeId::new(2).expect("return edge"),
        ]
    );
    let entry_bytes = entry.bytes(&object_artifact).to_vec();
    let entry_offset = u64::try_from(entry.text_offset).expect("terminal entry offset");
    let (installed_code, entry_stub) = install_terminal_object(
        &object_artifact,
        object_artifact.text_bytes().to_vec(),
        entry_offset,
    );
    let wrong_entry =
        EntryStubId::from_normalized_identity(0x5302).expect("different entry stub identity");
    let error = bind_installed_terminal_entry_fuel(
        fixed_fuel.clone(),
        &object_artifact,
        &installed_code,
        wrong_entry,
    )
    .expect_err("terminal fuel binding must reject a different installed entry");
    assert!(error.0.contains("selected installed entry"));
    let installed_fixed_fuel = bind_installed_terminal_entry_fuel(
        fixed_fuel.clone(),
        &object_artifact,
        &installed_code,
        entry_stub,
    )
    .expect("terminal fuel theorem should bind the exact installed source artifact");

    let sponsor_summary_identity =
        ProviderFuelSummaryId::from_normalized_identity(0x5350).expect("sponsor summary identity");
    let sponsor_provider =
        RootProviderId::from_normalized_identity(0x5351).expect("sponsor provider identity");
    let sponsor_work_receipt = ProviderFuelValidationReceiptId::from_normalized_identity(0x5352)
        .expect("sponsor work validation receipt");
    let sponsor_summary = FixedFuelProviderSummary::from_admitted_provider(
        sponsor_summary_identity,
        sponsor_provider,
        fixed_fuel.schedule(),
        1,
        BTreeSet::new(),
        sponsor_work_receipt,
    );
    let sponsor_demand = compose_fixed_fuel(sponsor_summary_identity, [&sponsor_summary])
        .expect("independent sponsor demand");
    let sponsor_suspension = AdmittedOpaqueFuelSuspensionFree::from_admitted_provider(
        sponsor_summary_identity,
        sponsor_provider,
        fixed_fuel.schedule(),
        sponsor_work_receipt,
        FuelSuspensionValidationReceiptId::from_normalized_identity(0x5353)
            .expect("sponsor suspension validation receipt"),
    );
    let sponsor_free = derive_fuel_suspension_free(&sponsor_demand, [sponsor_suspension])
        .expect("exhaustion sponsor path is suspension-free");
    let sponsor_fixed = admit_fixed_native_fuel(
        &sponsor_demand,
        FuelProvisionId::from_normalized_identity(0x5354).expect("sponsor fuel provision"),
        1,
    )
    .expect("exhaustion sponsor path has fixed provision");
    let sponsor_path = bind_suspension_free_fixed_fuel(sponsor_fixed, sponsor_free)
        .expect("fixed sponsor path binds its suspension evidence");
    let profile = omega_target::TargetProfile::host();
    assert_eq!(profile.native_target(), object_artifact.target());
    let context_register = match object_artifact.target().architecture {
        omega_target::Architecture::X86_64 => MachineRegister::X86Rbx,
        omega_target::Architecture::Aarch64 => MachineRegister::Aarch64X(28),
    };
    let (saved_general, saved_vector) = match object_artifact.target().architecture {
        omega_target::Architecture::X86_64 => (MachineRegister::X86Rax, MachineRegister::X86Xmm(0)),
        omega_target::Architecture::Aarch64 => {
            (MachineRegister::Aarch64X(0), MachineRegister::Aarch64V(0))
        }
    };
    let context = NativeFuelContextLayout {
        byte_size: 112,
        alignment: 16,
        remaining_units_offset: 0,
        unpaid_site_kind_offset: 8,
        unpaid_site_identity_offset: 16,
        required_units_offset: 24,
        transfer_entry_offset: 32,
        retry_code_offset_offset: 40,
        sponsor_stack_top_offset: 48,
        activation_state_offset: 64,
        activation_state_byte_count: 40,
    };
    let transfer_state = MachineStateSet::new([
        MachineState::GeneralRegisters,
        MachineState::VectorRegisters,
        MachineState::Flags,
        MachineState::InstructionPointer,
        MachineState::StackPointer,
    ]);
    let transfer_projection = NativeFuelTransferRuntimePlanProjection::new(
        profile,
        object_artifact.target(),
        SponsorContextTransport::ReservedNonvolatileRegister {
            register: context_register,
        },
        context,
        vec![
            NativeFuelActivationStateSlot {
                value: NativeFuelSavedValue::Register(saved_general),
                context_offset: 64,
                byte_count: 8,
            },
            NativeFuelActivationStateSlot {
                value: NativeFuelSavedValue::Flags,
                context_offset: 72,
                byte_count: 8,
            },
            NativeFuelActivationStateSlot {
                value: NativeFuelSavedValue::Register(saved_vector),
                context_offset: 80,
                byte_count: 16,
            },
            NativeFuelActivationStateSlot {
                value: NativeFuelSavedValue::StackPointer,
                context_offset: 96,
                byte_count: 8,
            },
        ],
        NativeFuelSponsorStackPlan {
            alignment: 16,
            byte_ceiling: 256,
        },
        transfer_state,
        transfer_state,
        transfer_state,
        NativeFuelRuntimeEntryIdentity {
            section_identity: 0x5356,
            symbol_identity: 0x5357,
        },
        NativeFuelRuntimeEntryIdentity {
            section_identity: 0x5356,
            symbol_identity: 0x5358,
        },
    )
    .expect("canonical host native fuel transfer projection");
    let target_policy = admit_native_fuel_target_policy(NativeFuelTargetPlanProjection {
        profile,
        target: object_artifact.target(),
        transport: transfer_projection.transport(),
        context: transfer_projection.context(),
        transfer_plan_identity: transfer_projection.normalized_identity(),
    })
    .expect("host native fuel target policy");
    let transfer_plan = admit_native_fuel_transfer_plan(target_policy, transfer_projection)
        .expect("host structural transfer plan matches target policy");
    let dynamic_plan = DynamicNativeFuelMeterPlan::from_admitted_transfer_plan(
        transfer_plan,
        fixed_fuel.schedule(),
        NativeFuelMeterPlanId::from_normalized_identity(0x5355).expect("native meter plan"),
        sponsor_path,
    );
    let missing_dynamic_attribution =
        validate_dynamic_fuel_attribution_basis(dynamic_plan, &object_artifact)
            .expect_err("native code without attribution rows cannot select dynamic metering");
    assert!(missing_dynamic_attribution.0.contains("at least one"));
    validate_installed_terminal_entry_fuel(&installed_fixed_fuel, &installed_code, entry_stub)
        .expect("external-root recheck should accept the exact installed code and entry");
    assert!(
        validate_installed_terminal_entry_fuel(&installed_fixed_fuel, &installed_code, wrong_entry)
            .is_err(),
        "external-root recheck must reject a different selected entry"
    );
    let fuel_summary_identity =
        ProviderFuelSummaryId::from_normalized_identity(0x5100).expect("fuel summary identity");
    let certified_summary = FixedFuelProviderSummary::from_terminal_entry(
        fuel_summary_identity,
        RootProviderId::from_normalized_identity(0x5200).expect("root provider identity"),
        installed_fixed_fuel,
        BTreeSet::new(),
    );
    let certified_demand = compose_fixed_fuel(fuel_summary_identity, [&certified_summary])
        .expect("installed terminal Psi should supply its hard-root local fuel demand");
    assert_eq!(certified_demand.schedule(), fixed_fuel.schedule());
    assert_eq!(certified_demand.units(), fixed_fuel.ceiling_units());
    assert!(
        certified_demand.provider_receipts().is_empty(),
        "a recomputable terminal-Psi certificate is not an opaque provider receipt"
    );
    let mut changed_bytes = object_artifact.text_bytes().to_vec();
    changed_bytes[0] ^= 1;
    let (changed_code, changed_entry) =
        install_terminal_object(&object_artifact, changed_bytes, entry_offset);
    assert!(
        bind_installed_terminal_entry_fuel(
            fixed_fuel.clone(),
            &object_artifact,
            &changed_code,
            changed_entry,
        )
        .is_err(),
        "terminal fuel evidence must reject different installed bytes"
    );
    let wrong_offset = if entry_offset == 0 { 4 } else { 0 };
    let (wrong_entry_code, wrong_entry) = install_terminal_object(
        &object_artifact,
        object_artifact.text_bytes().to_vec(),
        wrong_offset,
    );
    assert!(
        bind_installed_terminal_entry_fuel(
            fixed_fuel.clone(),
            &object_artifact,
            &wrong_entry_code,
            wrong_entry,
        )
        .is_err(),
        "terminal fuel evidence must reject a stub at the wrong function offset"
    );

    drop(machine_code);
    drop(target_operations);
    drop(abstract_operations);
    drop(verified);
    drop(semantic_module);
    drop(proof_bundle);

    let object = emit_terminal_object_container(&object_artifact);
    assert_eq!(object.terminal_psi, original_identity);
    assert_eq!(&object.output.bytes[..8], b"OMGOBJ\0\0");
    assert_eq!(object.output.text_bytes, object_artifact.text_bytes().len());
    assert_eq!(object.output.relocations, 0);
    let image = emit_terminal_executable_image(&object_artifact, 3)
        .expect("source-produced owned artifact should emit a standalone host image");
    assert_eq!(image.terminal_psi(), original_identity);
    assert_eq!(
        image.output().final_text_bytes,
        object_artifact.text_bytes()
    );
    assert!(
        image
            .output()
            .executable_regions
            .unclassified_gaps
            .is_empty()
    );
    let installation = build_terminal_installation_record(
        &image,
        ProfileDecisionId::new(1).expect("source installation profile decision"),
    )
    .expect("source image should produce a typed installation record");
    validate_terminal_installation_record(&installation, &image)
        .expect("installation record should bind the exact source image");
    let installation_bytes =
        encode_terminal_installation_record(&installation).expect("canonical installation bytes");
    let decoded_installation = decode_terminal_installation_record(&installation_bytes)
        .expect("canonical installation record should decode");
    assert_eq!(decoded_installation, installation);
    let decoded_stack_demand = derive_terminal_installation_stack_demand(
        &decoded_installation,
        &image,
        object_artifact.entry(),
    )
    .expect("decoded installation should reproduce its stack closure");
    assert_eq!(decoded_stack_demand, terminal_stack_demand);

    // A leaf may have a zero-byte internal closure; external-root admission
    // still needs a nonzero adapter/provision. Exercise the completed bridge
    // with a source-produced internal-call closure whose emitter-derived
    // demand is nonzero.
    let call_checked = compile_to_checked(&source_canary(), None)
        .expect("terminal call source canary should compile");
    let call_lowered = lower_machine(&call_checked, "terminal_call_forward")
        .expect("source internal call should lower to terminal Psi");
    let call_verified = verify_module(
        &call_lowered.semantic_module,
        &call_lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source internal-call terminal Psi should verify");
    let call_fuel = derive_fixed_entry_fuel(&call_verified, call_lowered.semantic_module.entry)
        .expect("source internal-call closure should have fixed fuel");
    let call_abstract = lower_verified_artifact(&call_verified)
        .expect("source internal-call closure should cross the Omega boundary");
    let call_target = lower_to_target_operations(&call_abstract, NativeTarget::host())
        .expect("source internal call should select for the host");
    let call_assigned = assign_registers(&call_target).expect("source call homes should assign");
    let call_machine_code =
        emit_machine_code(&call_assigned).expect("source internal call should emit");
    let object_artifact = build_terminal_object_artifact(&call_machine_code)
        .expect("source internal-call object artifact");
    let call_image = emit_terminal_executable_image(&object_artifact, 3)
        .expect("source internal-call executable image");
    let call_installation = build_terminal_installation_record(
        &call_image,
        ProfileDecisionId::new(2).expect("call installation profile"),
    )
    .expect("source internal-call installation record");
    let call_installation_bytes =
        encode_terminal_installation_record(&call_installation).expect("call installation bytes");
    let decoded_call_installation = decode_terminal_installation_record(&call_installation_bytes)
        .expect("call installation decode");
    let decoded_stack_demand = derive_terminal_installation_stack_demand(
        &decoded_call_installation,
        &call_image,
        object_artifact.entry(),
    )
    .expect("decoded call installation should reproduce its stack closure");
    assert!(decoded_stack_demand.ceiling_bytes() > 0);
    let entry_offset =
        u64::try_from(object_artifact.entry_function().text_offset).expect("call entry offset");
    let (mut installed_code, entry_stub) = install_terminal_object(
        &object_artifact,
        object_artifact.text_bytes().to_vec(),
        entry_offset,
    );
    let wrong_entry =
        EntryStubId::from_normalized_identity(0x5302).expect("different entry stub identity");
    let installed_fuel = bind_installed_terminal_entry_fuel(
        call_fuel,
        &object_artifact,
        &installed_code,
        entry_stub,
    )
    .expect("source call fuel should bind exact installed entry");
    let fuel_summary_identity = ProviderFuelSummaryId::from_normalized_identity(0x6100).unwrap();
    let certified_summary = FixedFuelProviderSummary::from_terminal_entry(
        fuel_summary_identity,
        RootProviderId::from_normalized_identity(0x5200).unwrap(),
        installed_fuel,
        BTreeSet::new(),
    );
    let certified_demand = compose_fixed_fuel(fuel_summary_identity, [&certified_summary])
        .expect("source call fixed-fuel composition");

    let installed_stack = bind_installed_terminal_entry_stack(
        &decoded_stack_demand,
        &object_artifact,
        &installed_code,
        entry_stub,
    )
    .expect("decoded terminal stack demand should bind exact installed bytes and entry");
    validate_installed_terminal_entry_stack(&installed_stack, &installed_code, entry_stub)
        .expect("installed terminal stack demand should revalidate");
    assert!(
        validate_installed_terminal_entry_stack(&installed_stack, &installed_code, wrong_entry)
            .is_err(),
        "terminal stack evidence must reject a different selected entry"
    );

    let root_identity = ExternalRootId::from_normalized_identity(0x6000).unwrap();
    let root_provider =
        RootProviderId::from_normalized_identity(0x5200).expect("root provider identity");
    let relation_identity = NestingRelationId::from_normalized_identity(0x6001).unwrap();
    let boundary = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(object_artifact.target()),
        &CallSignature {
            parameters: vec![omega_calling_conventions::ValueShape::integer(1, 1)],
            result: Some(omega_calling_conventions::ValueShape::integer(1, 1)),
        },
    )
    .expect("host external-root boundary");
    let stack_summary = ProviderStackSummary::from_terminal_entry(
        root_identity,
        root_provider,
        boundary.plan().state.stack,
        installed_stack,
    );
    let bound_stack = bind_direct_generated_entry_stack_realization(
        &stack_summary,
        &boundary,
        &installed_code,
        entry_stub,
        validate_entry_stack_domain_closure(
            boundary.plan().state.stack,
            vec![ArrivalContextStackDomain {
                context: ArrivalContextId::new(1).expect("arrival context"),
                domain: StackDomainRef::Interrupted,
            }],
        )
        .expect("host entry stack-domain closure"),
    )
    .expect("direct generated entry should derive its epoch realization");
    let composed_stack = compose_bound_entry_stack_epochs(
        &StackNestingRelation {
            identity: relation_identity,
            edges: BTreeSet::new(),
        },
        [&bound_stack],
    )
    .expect("terminal stack evidence should enter artifact-wide composition");
    let stack_input = composed_stack
        .input(root_identity)
        .expect("root stack input");
    assert!(matches!(
        stack_input.body_evidence(),
        omega_external_roots::StackLocalEvidence::TerminalEntry(binding)
            if binding.terminal_entry() == object_artifact.entry()
                && binding.installed_code() == installed_code.identity()
    ));
    assert_eq!(
        stack_input.realization_evidence().arrival_origin(),
        ArrivalStackRealizationOrigin::NoHardwareArrival
    );
    assert_eq!(
        stack_input.realization_evidence().adapter_origin(),
        AdapterStackRealizationOrigin::None
    );
    assert_eq!(
        stack_input.realization_evidence().validation_receipt(),
        None
    );
    let stack_ceiling = composed_stack
        .demand(root_identity)
        .expect("root stack demand")
        .domains()
        .map(|(_, demand)| demand.bytes)
        .max()
        .expect("direct entry has one stack domain");

    let trust_receipt = TrustReceiptId::from_normalized_identity(0x6002).unwrap();
    let candidate = ExternalRootCandidate {
        identity: root_identity,
        entry: entry_stub,
        provider: root_provider,
        provider_plan: ProviderPlanId::from_normalized_identity(0x6003).unwrap(),
        requirement_identity: "TerminalRoot::entry".into(),
        entry_claims: Vec::new(),
        acknowledgement_parameter_index: None,
        interrupt_mask_guard_claim: None,
        service_reach:
            omega_external_roots::ResolvedRootServiceReach::from_selected_provider_closure(
                Vec::new(),
                Vec::new(),
                &omega_effects::SelectedProviderPlanFacts::default(),
            )
            .expect("empty root service reach"),
        effects: BTreeSet::new(),
        trust_receipts: BTreeSet::from([trust_receipt]),
        nesting_relation: relation_identity,
        acknowledgement_policy: None,
        stack: StackResourceColumn {
            ceiling_bytes: stack_ceiling,
            realization: composed_stack,
            validation_receipt: StackValidationReceiptId::from_normalized_identity(0x6004).unwrap(),
        },
        logical_fuel: LogicalFuelResourceColumn {
            schedule: certified_demand.schedule(),
            provision: FuelProvisionId::from_normalized_identity(0x6005).unwrap(),
            ceiling_units: certified_demand.units(),
            realization: certified_demand,
            validation_receipt: FuelValidationReceiptId::from_normalized_identity(0x6006).unwrap(),
        },
        machine_state: MachineStateResourceColumn {
            realization: StateFootprintEvidence::new(
                RegisterSet::new([]),
                MachineStateSet::empty(),
            ),
            validation_receipt: StateValidationReceiptId::from_normalized_identity(0x6007).unwrap(),
        },
        component_pins: BTreeSet::new(),
    };
    let validated_root =
        validate_external_root(candidate, &boundary).expect("terminal-backed root validation");
    let provider_execution = ProviderExecution::from_admitted_provider(
        ProviderExecutionId::from_normalized_identity(0x6008).unwrap(),
        &validated_root,
        Some(OpaqueProviderExitAssurance::HardwareIsolation {
            validation_receipt: trust_receipt,
        }),
    )
    .expect("terminal-backed provider execution");
    let slot = RootSlotAuthority::from_admitted_owner(
        RootSlotId::from_normalized_identity(0x6009).unwrap(),
        RootSlotOwnerId::from_normalized_identity(0x600a).unwrap(),
    );
    let admission = RootAdmission::from_admitted_provider(
        RootAdmissionId::from_normalized_identity(0x600b).unwrap(),
        &validated_root,
        &provider_execution,
        &installed_code,
        &slot,
        [trust_receipt],
    )
    .expect("terminal-backed root admission");
    let mut ledger =
        InstalledRootLedger::claim(&mut installed_code).expect("canonical root ledger");
    let _installed_root = ledger
        .install(&installed_code, validated_root, slot, admission)
        .expect("terminal stack evidence should reach the installed-root report");
    let root_record = ledger.record(root_identity).expect("installed root record");
    assert_eq!(
        root_record
            .stack
            .realization
            .input(root_identity)
            .expect("installed root stack input")
            .pure()
            .body_wcsu_bytes,
        decoded_stack_demand.ceiling_bytes()
    );
    assert!(matches!(
        root_record
            .stack
            .realization
            .input(root_identity)
            .expect("reported root stack input")
            .body_evidence(),
        omega_external_roots::StackLocalEvidence::TerminalEntry(binding)
            if binding.artifact() == installed_code.artifact()
    ));
    let root_report = external_root_manifest_json(&ledger);
    assert!(root_report.contains("\"origin\": \"terminal_entry\""));
    assert!(root_report.contains("\"arrival_origin\": \"no_hardware_arrival\""));
    assert!(root_report.contains("\"adapter_origin\": \"none\""));
    assert!(root_report.contains("\"contributing_machines\": ["));

    let manifest_module = decode_module(&canonical_bytes)
        .expect("redecode semantic bytes after image realization state is dropped");
    let manifest_proof = decode_proof_bundle(&canonical_proof_bytes)
        .expect("redecode proof bytes after image realization state is dropped");
    let installed_manifest = build_artifact_manifest(
        &manifest_module,
        &manifest_proof,
        Some(&installation_bytes),
        None,
    )
    .expect("typed installation bytes should enter the artifact manifest");
    validate_artifact_manifest(
        &manifest_module,
        &manifest_proof,
        Some(&installation_bytes),
        None,
        installed_manifest,
    )
    .expect("installed artifact manifest should recompute from canonical sections");
    assert_eq!(installed_manifest.semantic(), original_identity);
    assert!(installed_manifest.installation().is_some());
    assert_ne!(installed_manifest.identity(), artifact_manifest.identity());

    let expected_exit = match interpreted {
        TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
            value: IntegerValue::Signed(value),
            ..
        }) => i32::try_from(value).expect("source canary exit fits i32"),
        other => panic!("source canary returned unexpected value {other:?}"),
    };
    assert_eq!(run_host_machine_code(&entry_bytes), expected_exit);
    #[cfg(target_os = "macos")]
    assert_eq!(
        run_host_executable_image(&image.output().bytes),
        expected_exit
    );
}
