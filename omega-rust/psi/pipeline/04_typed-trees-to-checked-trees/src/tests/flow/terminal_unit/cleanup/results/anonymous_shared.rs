//! Shared temporary access retains one real owner until the call continuation.
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;
use language_semantics::{PermissionAccess, PermissionEventSource, PermissionProvenance};

fn source(boundary: bool, following: &str) -> checked_trees::CheckedTrees {
    let (producer, parameters, argument, reach) = if boundary {
        (
            "boundary trait Factory { machine create() -> Token reaches Factory; }",
            "",
            "Factory::create()",
            "reaches Factory",
        )
    } else {
        (
            "machine forward(token: Token) -> Token { token }",
            "token: Token",
            "forward(token)",
            "",
        )
    };
    checked(&format!(
        r#"
        pub data Token {{ value: u64; }}
        {producer}
        machine read(token: &Token) {{}}
        machine finish() {{}}
        machine main({parameters}) {reach} {{ read(&{argument}); {following} }}
    "#
    ))
}

#[test]
fn anonymous_shared_permissions_reject_missing_changed_and_late_custody() {
    for boundary in [false, true] {
        let original = source(boundary, "");
        let machine = machine_named(&original, "main");
        assert!(
            original
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(machine)
                .is_some()
        );
        let events = original
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter(|(_, event)| {
                event.machine_symbol == machine
                    && matches!(event.root, facts::PlaceRoot::Expression(_))
            })
            .map(|(handle, event)| (handle, event.clone()))
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 3);
        for (handle, event) in events {
            for mutation in [
                "missing",
                "duplicate",
                "root",
                "provenance",
                "late",
                "access",
            ] {
                let mut changed = original.clone();
                let permissions = &mut changed.facts.flow.ownership.permissions;
                match mutation {
                    "missing" => {
                        assert!(permissions.free(handle));
                    }
                    "duplicate" => {
                        permissions.insert(event.clone());
                    }
                    "root" => permissions.get_mut(handle).root = facts::PlaceRoot::Unknown,
                    "provenance" => {
                        permissions.get_mut(handle).provenance = PermissionProvenance::Unknown
                    }
                    "late" => permissions.get_mut(handle).source = PermissionEventSource::StateExit,
                    "access" => permissions.get_mut(handle).access = PermissionAccess::Exclusive,
                    _ => unreachable!(),
                }
                if let Ok(changed) = crate::settle_checked_execution(changed) {
                    assert!(
                        changed
                            .facts
                            .flow
                            .terminal_unit_effects
                            .for_machine(machine)
                            .is_none(),
                        "{mutation} {:?}, boundary={boundary}",
                        event.access
                    );
                }
            }
        }
        let later = source(boundary, "finish();");
        assert!(
            later
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(machine_named(&later, "main"))
                .is_some(),
            "temporary cleanup precedes the following statement"
        );
    }
}
