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
fn checked_crash_branches_execute_but_native_graph_crashes_remain_unsupported() {
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
        assert!(lower_to_target_operations(&abstract_operations, target).is_err());
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
