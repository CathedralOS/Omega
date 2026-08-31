use super::*;

fn render(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn lifecycle_operations_conserve_the_linear_claim() {
    let pass = pass_canary("core/task_lifecycle_operations");
    let checked = compile_to_checked(&pass.join("main.omg"), None).unwrap_or_else(|diagnostics| {
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
            psi_language_semantics::MachineSupplyMode::TopLevelRequirement,
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

    let fail = fail_canary("core/task_core_scope_loss");
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
fn parked_continuation_is_not_source_addressable_through_task_claims() {
    for operation in ["projection", "recast", "address", "mutation"] {
        let name = format!("core/task_parked_continuation_{operation}_rejected");
        let diagnostics =
            check_canary(&fail_canary(&name)).expect_err("parked continuation access must reject");
        let rendered = render(&diagnostics);
        assert!(
            rendered.contains("has no field `continuation`"),
            "expected compiler-owned continuation opacity for {operation}, got:\n{rendered}"
        );
    }
}
