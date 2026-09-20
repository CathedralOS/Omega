use super::{
    Diagnostic, check_canary, compile_canary_without_output, compile_reviewed_repository_fixture,
    fail_canary, pass_canary,
};
use compiler::CheckedCompileRequest;

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
