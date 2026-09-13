//! Source-produced canonical `ProcessExit::exit_process` status: the exact
//! toolchain-owned requirement, its provider row, the checked interpreter's
//! simulated-domain halt with ordered output before the terminal event, and
//! the retained native image's physical exit on each hosted target.
use super::*;

#[test]
fn process_exit_canary_interprets_with_exact_status_and_ordered_output() {
    let canary = pass_canary(fixture_roster::RUNTIME_PROCESS_EXIT_I32_STATUS_ORDERED);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("canonical ProcessExit canary should reach checked trees");

    // The canonical schema row is the toolchain-owned `ProcessExit` trait's
    // exact requirement, selected under its own binding -- not a Console row.
    let process_exit_plans = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .zip(checked.selected_provider_provenance())
        .filter(|(plan, _)| plan.schema.trait_name == "ProcessExit")
        .collect::<Vec<_>>();
    let [(process_exit_plan, provenance)] = process_exit_plans.as_slice() else {
        panic!("exactly one selected ProcessExit provider plan");
    };
    assert!(
        process_exit_plan.schema.trait_package_identity.is_none(),
        "the canonical ProcessExit trait is toolchain-owned, not package-owned"
    );
    assert_eq!(
        provenance.row_compiler_intrinsic_executions.as_slice(),
        &[None],
        "a targetless check recognizes the requirement without closing a target execution"
    );

    // The simulated domain ends with the exact status; output written before
    // the terminal event is retained and nothing after it runs.
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.error, None,
        "canonical ProcessExit should halt the simulated domain cleanly"
    );
    assert_eq!(outcome.exit_code, 70);
    assert_eq!(outcome.stdout, b"process-exit canary\n".to_vec());
}

#[test]
fn process_exit_canary_interprets_native_case_with_exact_status() {
    let canary = pass_canary(fixture_roster::RUNTIME_PROCESS_EXIT_I32_STATUS);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("canonical ProcessExit native case should reach checked trees");

    // The simulated domain ends with the exact status the taken conditional
    // branch passes to the canonical requirement; no output is produced by
    // this ProcessExit-only receiver.
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.error, None,
        "canonical ProcessExit should halt the simulated domain cleanly"
    );
    assert_eq!(outcome.exit_code, 70);
    assert!(outcome.stdout.is_empty());
}

#[test]
fn process_exit_normalizes_source_i32_status_through_selected_custody() {
    let canary = pass_canary(fixture_roster::RUNTIME_PROCESS_EXIT_I32_STATUS);
    for target in ["linux_x86_64", "linux_arm64", "macos_arm64"] {
        let report = compile_rooted_backend_canary_without_output_for_target(&canary, target)
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "{target} canonical ProcessExit status reaches hosted exit: {diagnostics:#?}"
                )
            });
        let artifact = report
            .retained_native_artifact()
            .expect("source compilation retains native custody");
        let settlements = artifact.image().boundary_settlements();
        let exit_settlements = settlements
            .iter()
            .filter(|settlement| {
                settlement.settlement.execution
                    == native_realization::BoundaryExecutionRecord::CompilerBuiltin(
                        target_operations::CompilerBuiltinExecution::HostedExitProcessI32,
                    )
            })
            .collect::<Vec<_>>();
        // Each conditional exit helper settles its own exact boundary row:
        // `good` (status 70) and `bad` (status 1) keep independent argument
        // custody rather than sharing a manufactured terminal tail.
        let [good_settlement, bad_settlement] = exit_settlements.as_slice() else {
            panic!(
                "one exact hosted exit boundary per conditional branch for {target}; \
                 settlements: {:#?}",
                settlements
                    .iter()
                    .map(|settlement| format!("{:?}", settlement.settlement))
                    .collect::<Vec<_>>()
            );
        };
        for settlement in [good_settlement, bad_settlement] {
            assert!(settlement.settlement.native_result.is_unit());
            assert_eq!(settlement.settlement.runtime_scalar_arguments.len(), 1);
            assert!(matches!(
                settlement.settlement.runtime_scalar_arguments[0].source,
                machine_code::InternalUnitScalarArgumentSourceRecord::SelectedProcessExit { .. }
            ));
            assert!(
                settlement.settlement.scalar_arguments.is_empty(),
                "the scalar definition is an actual runtime carrier, not a manufactured immediate"
            );
        }
        let selected_source_value =
            |settlement: &machine_code::BoundarySettlementRecord| match settlement
                .runtime_scalar_arguments[0]
                .source
            {
                machine_code::InternalUnitScalarArgumentSourceRecord::SelectedProcessExit {
                    source_value,
                    ..
                } => source_value,
                ref other => panic!("non-ProcessExit exit argument custody: {other:?}"),
            };
        assert_ne!(
            selected_source_value(&good_settlement.settlement),
            selected_source_value(&bad_settlement.settlement),
            "each exit branch retains its own exact status custody for {target}"
        );
        assert!(artifact.physical_evidence().is_some());
        hosted_exit::assert_matching_host_exit_stdout(
            target,
            &artifact.image().output().bytes,
            70,
            b"",
        );
    }
}
