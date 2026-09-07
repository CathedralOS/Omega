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
    *crash = checked_trees::CrashPlan::published_ceiling(crash.published().to_vec());
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
    let uncovered_site = checked_trees::CheckedCrashSite::new(
        site.location(),
        site.cause(),
        Vec::new(),
        site.frontier_lower_bound().to_vec(),
    );
    *crash = checked_trees::CrashPlan::published_ceiling(crash.published().to_vec())
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
    let claim = language_semantics::PermissionClaimIdentity::Established {
        machine_symbol: terminal_abort,
        state_symbol: site.location().state(),
        source: language_semantics::PermissionEventSource::StateEntry,
        ordinal: 0,
    };
    let site_with_frontier = checked_trees::CheckedCrashSite::new(
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
            .any(|operation| matches!(operation, AbstractOperation::Crash { .. }))
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("guarded Boolean crash should select as mixed terminal control");
        let TargetOperation::ReturnBooleanConditionalControl {
            when_true,
            when_false,
            ..
        } = &target_operations.functions[0].operation
        else {
            panic!("direct Boolean guard should retain target conditional control");
        };
        assert!(matches!(
            when_true.control.as_ref(),
            TargetBooleanControl::Crash {
                cause: CrashCause::Trap,
                site_guard,
                frontier_lower_bound,
                ..
            } if !site_guard.is_empty()
                && frontier_lower_bound.is_empty()
        ));
        assert!(matches!(
            when_false.control.as_ref(),
            TargetBooleanControl::ReturnImmediate { value: true, .. }
        ));
    }
}

#[test]
fn target_lowering_preserves_every_reachable_crash_leaf() {
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
        let TargetOperation::ReturnIntegerConditionalControl {
            when_true,
            when_false,
            ..
        } = &target_operations.functions[0].operation
        else {
            panic!("direct Boolean guard should retain integer target control");
        };
        assert!(matches!(
            when_true.control.as_ref(),
            TargetIntegerControl::Crash {
                cause: CrashCause::Trap,
                site_guard,
                frontier_lower_bound,
                ..
            } if !site_guard.is_empty()
                && frontier_lower_bound.is_empty()
        ));
        assert!(matches!(
            when_false.control.as_ref(),
            TargetIntegerControl::Return { .. }
        ));
    }
    let integer_guarded_trap = lower_machine(&checked, "terminal_integer_guarded_trap")
        .expect("exact-type integer comparison should open a guarded crash branch");
    assert!(matches!(
        &integer_guarded_trap.semantic_module.machines[0].blocks[0].operations[..],
        [
            terminal_psi::Operation {
                kind: terminal_psi::OperationKind::IntegerConstant { .. },
                ..
            },
            terminal_psi::Operation {
                kind: terminal_psi::OperationKind::WrappingIntegerAdd { .. },
                ..
            },
            terminal_psi::Operation {
                kind: terminal_psi::OperationKind::IntegerLessOrEqual { left, right },
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
    assert_guarded_crash_lowers(&integer_guarded_verified);
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
    assert_guarded_crash_lowers(&transitive_verified);
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
    assert_guarded_crash_lowers(&implied_verified);
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
            [AbstractOperation::Crash {
                cause,
                site_guard,
                frontier_lower_bound,
                ..
            }] if *cause == expected_cause
                && site_guard.is_empty()
                && frontier_lower_bound.is_empty()
        ));

        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let target_operations = lower_to_target_operations(&abstract_operations, target)
                .expect("unconditional crash should select");
            assert!(matches!(
                &target_operations.functions[0].operation,
                TargetOperation::Crash {
                    cause,
                    site_guard,
                    frontier_lower_bound,
                    ..
                } if *cause == expected_cause
                    && site_guard.is_empty()
                    && frontier_lower_bound.is_empty()
            ));
        }
    }
}

#[cfg(unix)]
#[test]
fn interpreted_terminal_source_matches_target_lowering() {
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
    let optimization = terminal_codec::build_identity_optimization_execution_record(
        &lowered.semantic_module,
        &lowered.proof_bundle,
    )
    .expect("source-produced identity optimization execution");
    let artifact_manifest = build_artifact_manifest(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &optimization,
        None,
        None,
    )
    .expect("source-produced terminal sections should have a manifest");
    drop(lowered);
    let semantic_module = decode_module(&canonical_bytes)
        .expect("canonical source-produced terminal Psi should decode");
    let proof_bundle = decode_proof_bundle(&canonical_proof_bytes)
        .expect("canonical source-produced proof bundle should decode");
    validate_artifact_manifest(
        &semantic_module,
        &proof_bundle,
        &optimization,
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
    let _interpreted = match execution.resume(&mut meter).unwrap() {
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
    let _target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("constant terminal requirements should select for the host");
}
