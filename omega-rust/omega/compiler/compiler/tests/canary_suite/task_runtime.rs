use super::{
    Diagnostic, check_canary, compile_canary_without_output, compile_reviewed_repository_fixture,
    compile_to_checked, fail_canary, pass_canary, repo_root, repository_fixture_package_inputs,
};
use compiler::CheckedCompileRequest;
use std::fs;

#[path = "../fixture_rosters/task_runtime.rs"]
pub(super) mod fixture_roster;

fn render(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn lifecycle_operations_conserve_the_linear_claim() {
    let pass = pass_canary(fixture_roster::CORE_TASK_LIFECYCLE_OPERATIONS);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &pass.join("main.omg"),
        None,
    ))
    .unwrap_or_else(|diagnostics| {
        panic!(
            "task lifecycle operations should conserve the claim:\n{}",
            render(&diagnostics)
        )
    });
    let snapshot = checked.typed.snapshot();
    for (name, suspends, blocks) in [
        ("Task::request_cancel", false, false),
        ("Task::finish", true, true),
    ] {
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("checked-in core requirement `{name}`"));
        assert!(machine.is_public, "{name}");
        assert_eq!(
            machine.supply_mode,
            language_semantics::MachineSupplyMode::TopLevelRequirement,
            "{name}"
        );
        assert!(!machine.body_is_present, "{name}");
        let machine_snapshot = snapshot
            .roots
            .machines
            .iter()
            .find(|machine| machine.name == name)
            .unwrap_or_else(|| panic!("snapshot for `{name}`"));
        assert_eq!(machine_snapshot.suspends, suspends, "{name}");
        assert_eq!(machine_snapshot.blocks, blocks, "{name}");
        assert!(machine_snapshot.contracts.is_empty(), "{name}");
    }

    let fail = fail_canary(fixture_roster::CORE_TASK_CORE_SCOPE_LOSS);
    let diagnostics = compile_canary_without_output(&fail)
        .expect_err("request_cancel must not settle the task claim");
    let rendered = render(&diagnostics);
    assert!(
        rendered.contains(
            "linear value `task` reaches scope exit without being consumed or transferred"
        ),
        "request_cancel should preserve the original live claim:\n{rendered}"
    );
}

#[test]
fn blocking_executor_custody_claims_hold_through_the_concrete_pin() {
    // BLOCKEXEC concrete pin: the blocking-executor contract surface
    // (bounded submissions, moved custody, linear completion claims,
    // suspension at the wait substrate, provider selection) instantiated
    // at `Token`, since queue/executor machine bodies sit at the
    // generic-machine frontier and real requirement admission rides the
    // TR3-TR8 provider route.
    let pass = pass_canary(fixture_roster::BLOCKEXEC_BLOCKING_EXECUTOR_CUSTODY_CLAIMS_COMPILE);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &pass.join("main.omg"),
        None,
    ))
    .unwrap_or_else(|diagnostics| {
        panic!(
            "blocking-executor custody pin should reach checked trees:\n{}",
            render(&diagnostics)
        )
    });

    // Linear claims: submissions, completion tickets, and the executor
    // itself are exactly-once values.
    for name in ["Submission", "Ticket", "Executor"] {
        let definition = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == name)
            .unwrap_or_else(|| panic!("checked-in `{name}` declaration"));
        assert_eq!(
            definition.properties.multiplicity,
            language_semantics::Multiplicity::Linear,
            "{name} must carry the `[linear]` claim"
        );
    }

    // Operational envelopes: settlement parks at the wait substrate, the
    // probe carries both suspension and blocking, and plain helpers carry
    // neither.
    let snapshot = checked.typed.snapshot();
    for (name, suspends, blocks, supply, body) in [
        (
            "Ticket::settle",
            true,
            true,
            language_semantics::MachineSupplyMode::CheckedBody,
            true,
        ),
        (
            "WaitSubstrate::park",
            true,
            true,
            language_semantics::MachineSupplyMode::Boundary,
            false,
        ),
        (
            "WaitSubstrate::wake_one",
            false,
            false,
            language_semantics::MachineSupplyMode::Boundary,
            false,
        ),
        (
            "Submission::release",
            false,
            false,
            language_semantics::MachineSupplyMode::CheckedBody,
            true,
        ),
        (
            "Main::probe",
            true,
            true,
            language_semantics::MachineSupplyMode::CheckedBody,
            true,
        ),
    ] {
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("checked-in machine `{name}`"));
        assert_eq!(machine.supply_mode, supply, "{name}");
        assert_eq!(machine.body_is_present, body, "{name}");
        let machine_snapshot = snapshot
            .roots
            .machines
            .iter()
            .find(|machine| machine.name == name)
            .unwrap_or_else(|| panic!("snapshot for `{name}`"));
        assert_eq!(machine_snapshot.suspends, suspends, "{name}");
        assert_eq!(machine_snapshot.blocks, blocks, "{name}");
    }

    // Provider selection: build composition admits the canary worker
    // provider for the executor's `WorkerProvider` schema.
    let plan = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .find(|plan| plan.schema.trait_name == "WorkerProvider")
        .expect("selected WorkerProvider plan");
    assert_eq!(plan.provider_type, "CanaryWorkerProvider");
    assert!(
        plan.rows
            .iter()
            .any(|row| row.requirement_identity.contains("execute")),
        "the selected plan must carry the worker `execute` row"
    );
}

#[test]
fn blocking_executor_package_checks_with_the_closed_service_carrier() {
    // BLOCKEXEC package pin: the bundled `blocking-executor` package itself
    // must reach checked trees — its `Executor.runtime` field holds the
    // plain `Binding<WorkerProvider>` closed carrier, which an authored
    // `in Bound` qualification would reject outright.
    let package_root = repo_root().join("source/library/blocking-executor/main.omg");
    let checked =
        compile_reviewed_repository_fixture(CheckedCompileRequest::new(&package_root, None))
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "blocking-executor package should reach checked trees:\n{}",
                    render(&diagnostics)
                )
            });

    let executor = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Executor")
        .expect("checked-in `Executor` declaration");
    assert_eq!(
        executor.properties.multiplicity,
        language_semantics::Multiplicity::Linear,
        "Executor must carry the `[linear]` claim"
    );
}

#[test]
fn blocking_executor_linear_slots_swap_whole_through_the_concrete_pin() {
    // BLOCKEXEC slot-empty channel leg: the package's `BoundedQueue<T, N>`
    // ring over per-slot linear payloads is pinned concretely — a
    // sum-typed slot (`Empty`/`Held`) gives the claim-free channel the
    // row records as missing, and the whole-slot take/restore idiom
    // checks: `take` moves the `Held` slot out whole and writes `Empty`
    // back so the claim reaches the caller exactly once, `put` writes a
    // fresh `Held` over a vacated slot. Head-indexed ring arithmetic
    // stays on the contract-fold lane (see the fixture header).
    let pass = pass_canary(fixture_roster::BLOCKEXEC_LINEAR_SLOT_SWAP_COMPILE);
    compile_reviewed_repository_fixture(CheckedCompileRequest::new(&pass.join("main.omg"), None))
        .unwrap_or_else(|diagnostics| {
            panic!(
                "blocking-executor linear slot pin should reach checked trees:\n{}",
                render(&diagnostics)
            )
        });
}

#[test]
fn parked_continuation_is_not_source_addressable_through_task_claims() {
    for &(operation, name) in fixture_roster::PARKED_CONTINUATION_FAIL_CANARIES {
        let diagnostics =
            check_canary(&fail_canary(name)).expect_err("parked continuation access must reject");
        let rendered = render(&diagnostics);
        assert!(
            rendered.contains("has no field `continuation`"),
            "expected compiler-owned continuation opacity for {operation}, got:\n{rendered}"
        );
    }
}

#[test]
fn blocking_executor_consumer_compiles_through_the_depend_edge() {
    // BLOCKEXEC depend-edge leg: the consumer fixture's authored
    // `builder.depend` row reaches the bundled blocking-executor package
    // through real package inputs, so `use blocking_executor::executor`
    // resolves the package's own contract declarations.
    let pass = pass_canary(fixture_roster::BLOCKEXEC_BLOCKING_EXECUTOR_CONSUMER_DEPEND_COMPILE);
    let root = pass.join("main.omg");
    let package_inputs =
        repository_fixture_package_inputs(&root).expect("the depend edge wires package inputs");
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs),
        ..CheckedCompileRequest::new(&root, None)
    })
    .unwrap_or_else(|diagnostics| {
        panic!(
            "blocking-executor consumer should reach checked trees:\n{}",
            render(&diagnostics)
        )
    });

    // The package's own contract types arrive through the depend edge.
    for name in ["Submission", "Ticket", "Executor", "BoundedQueue"] {
        let definition = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == name)
            .unwrap_or_else(|| panic!("checked-in `{name}` declaration"));
        assert_eq!(
            definition.properties.multiplicity,
            language_semantics::Multiplicity::Linear,
            "{name} must carry the `[linear]` claim"
        );
    }

    // Provider selection: the consumer's canary provider satisfies the
    // package's `WorkerProvider` trait through ordinary build composition.
    let plan = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .find(|plan| plan.schema.trait_name == "WorkerProvider")
        .expect("selected WorkerProvider plan");
    assert_eq!(plan.provider_type, "CanaryWorkerProvider");
}

#[test]
fn blocking_executor_isolated_provider_conformance_binds_the_inherited_row() {
    // BLOCKEXEC isolated-provider composition leg: the package declares
    // `IsolatedWorkerProvider: WorkerProvider` as the composition a
    // worker-isolating provider satisfies. The marker trait owns no
    // requirements, so its complete conformance surface is the inherited
    // `WorkerProvider::execute` row — the fixture binds it by reference to
    // the satisfies-clause realization, and the incomplete-conformance twin
    // (FAIL_CANARIES) pins the missing-row rejection.
    let pass = pass_canary(
        fixture_roster::BLOCKEXEC_BLOCKING_EXECUTOR_ISOLATED_PROVIDER_CONFORMANCE_COMPILE,
    );
    let root = pass.join("main.omg");
    let package_inputs =
        repository_fixture_package_inputs(&root).expect("the depend edge wires package inputs");
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs),
        ..CheckedCompileRequest::new(&root, None)
    })
    .unwrap_or_else(|diagnostics| {
        panic!(
            "blocking-executor isolated-provider consumer should reach checked trees:\n{}",
            render(&diagnostics)
        )
    });

    // The named conformance to the package's marker trait is retained with
    // the canary provider as its carrier.
    let conformance = checked
        .conformances()
        .iter()
        .find(|conformance| conformance.trait_name.as_str() == "IsolatedWorkerProvider")
        .expect("checked-in `IsolatedWorkerProvider` conformance");
    assert_eq!(
        conformance.carrier_name().map(|name| name.as_str()),
        Some("CanaryIsolatedWorkerProvider"),
        "the marker-trait conformance must name the canary provider as carrier"
    );

    // The same provider is still selectable for the inherited `WorkerProvider`
    // slot through ordinary build composition.
    let plan = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .find(|plan| plan.schema.trait_name == "WorkerProvider")
        .expect("selected WorkerProvider plan");
    assert_eq!(plan.provider_type, "CanaryIsolatedWorkerProvider");

    // The fence twin: an `IsolatedWorkerProvider` conformance binding no row
    // rejects as incomplete — ambient machines never fill trait rows.
    let fail = fail_canary(
        fixture_roster::BLOCKEXEC_BLOCKING_EXECUTOR_ISOLATED_PROVIDER_INCOMPLETE_CONFORMANCE,
    );
    let expected = fs::read_to_string(fail.join("expected.txt"))
        .expect("incomplete-conformance fail canary should carry expected.txt");
    let fail_root = fail.join("main.omg");
    let fail_package_inputs = repository_fixture_package_inputs(&fail_root)
        .expect("the depend edge wires package inputs");
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(fail_package_inputs),
        ..CheckedCompileRequest::new(&fail_root, None)
    })
    .expect_err("an empty IsolatedWorkerProvider conformance must reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains(expected.trim()),
        "{} missing expected fragment {:?}:\n{combined}",
        fail.display(),
        expected.trim()
    );
}
