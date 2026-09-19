use crate::tests::{checked_source, lower_machine};
use terminal_psi::ClosedConformanceCallableResult;

const MIXED_RESULTS: &str = r#"
    trait Device {
        machine ready(&self) -> bool;
        machine touch(&self);
    }
    data Item [copy] { ready: bool; }
    Primary: Item satisfies Device {
        machine ready(&self) -> bool { transition { _ -> self.ready } }
        machine touch(&self) {}
    }
    data Main [copy] { item: Item; }
    machine forward_query(erased: &dyn Device) -> bool {
        let result: bool = erased.ready();
        transition { _ -> result }
    }
    machine forward_command(erased: &dyn Device) { erased.touch(); }
"#;

const CALLS: [&str; 2] = [
    "let result: bool = forward_query(erased);",
    "forward_command(erased);",
];

fn source(call: &str) -> String {
    format!(
        "{MIXED_RESULTS}\n machine Main::run(&self) {{
        let erased: &dyn Device = &self.item as &dyn Item::Primary;
        {call}
    }}"
    )
}

#[test]
fn dynamic_table_retains_each_members_result_kind() {
    for call in CALLS {
        let checked = checked_source(&source(call));
        let lowered = lower_machine(&checked, "Main::run")
            .expect("query and command use the same complete mixed-result conformance");
        let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::run")
            .produce_artifact()
            .expect("mixed-result table verifies and serializes");
        let decoded = terminal_codec::decode_module(artifact.semantic_bytes())
            .expect("mixed-result table decodes");
        assert_eq!(decoded, lowered.semantic_module);
        let [application] = decoded.closed_conformance_applications.as_slice() else {
            panic!("one explicitly selected conformance")
        };
        assert_eq!(application.rows.len(), 2);
        assert_eq!(application.realization_callables.len(), 2);
        assert!(
            application
                .realization_callables
                .iter()
                .any(|callable| callable.result == ClosedConformanceCallableResult::Unit)
        );
        assert!(
            application
                .realization_callables
                .iter()
                .any(|callable| callable.result == ClosedConformanceCallableResult::Bool)
        );
        super::plan_isolation::assert_supported_artifact_executes(&artifact, &decoded);
    }
}

#[test]
fn mixed_table_rejects_result_kind_drift_in_called_and_uncalled_members() {
    for call in CALLS {
        let checked = checked_source(&source(call));
        for member in 0..2 {
            let mut changed = checked.clone();
            let catalog = &mut changed.facts.flow.terminal_unit_effects.dynamic_dispatch;
            let callables = if let [plan] = catalog.direct_scalar_calls.as_mut_slice() {
                &mut plan.realization_callables
            } else if let [plan] = catalog.direct_unit_calls.as_mut_slice() {
                &mut plan.realization_callables
            } else {
                panic!("one mixed-table call plan")
            };
            callables[member].body = callables[1 - member].body.clone();
            assert!(
                lower_machine(&changed, "Main::run").is_err(),
                "member {member} cannot adopt another member's result kind: {call}"
            );
        }
    }
}

#[test]
fn mixed_table_cannot_omit_an_uncalled_member() {
    for call in CALLS {
        let mut checked = checked_source(&source(call));
        let catalog = &mut checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
        if let [plan] = catalog.direct_scalar_calls.as_mut_slice() {
            plan.realization_callables
                .retain(|row| row.realization_machine == plan.realization_machine);
        } else if let [plan] = catalog.direct_unit_calls.as_mut_slice() {
            plan.realization_callables
                .retain(|row| row.realization_machine == plan.realization_machine);
        } else {
            panic!("one mixed-table call plan")
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&checked, "Main::run")
                .produce_artifact()
                .is_err(),
            "a forwarded descriptor requires its complete table"
        );
    }
}
