//! Temporary projections retain exact producer, transfer, and residual facts.
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;
use language_semantics::{PermissionEventKind, PermissionEventSource, PermissionProvenance};

fn source(boundary: bool) -> checked_trees::CheckedTrees {
    let (producer, parameters, expression, reach) = if boundary {
        (
            "boundary trait Factory { machine create() -> Pair reaches Factory; }",
            "",
            "Factory::create()",
            "reaches Factory",
        )
    } else {
        (
            "machine forward(value: Pair) -> Pair { value }",
            "value: Pair",
            "forward(value)",
            "",
        )
    };
    checked(&format!(
        r#"
        pub data Token {{ value: u64; }}
        pub data Pair {{ left: Token; right: Token; }}
        data Sink {{}}
        machine Sink::take(value: Token) {{}}
        {producer}
        data Main {{}}
        machine Main::enter({parameters}) {reach} {{
            Sink::take({expression}.right);
        }}
    "#
    ))
}

#[test]
fn anonymous_projection_permissions_cannot_be_removed_duplicated_or_rebound() {
    for boundary in [false, true] {
        let original = source(boundary);
        let machine = machine_named(&original, "enter");
        assert!(
            original
                .facts
                .flow
                .terminal_partial_affine_unit_cleanups
                .for_machine(machine)
                .is_some()
        );
        let rows = original
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
        assert_eq!(rows.len(), 3);
        for (handle, event) in rows {
            for mutation in [
                "missing",
                "duplicate",
                "root",
                "path",
                "provenance",
                "coordinate",
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
                    "path" => {
                        permissions.get_mut(handle).segments = if event.segments.is_empty() {
                            original
                                .facts
                                .flow
                                .ownership
                                .permissions
                                .iter()
                                .map(|(_, event)| event)
                                .find(|event| {
                                    event.machine_symbol == machine
                                        && event.kind == PermissionEventKind::AffineDrop
                                })
                                .unwrap()
                                .segments
                        } else {
                            arena::HandleSpan::empty()
                        };
                    }
                    "provenance" => {
                        permissions.get_mut(handle).provenance = PermissionProvenance::Unknown
                    }
                    "coordinate" => {
                        permissions.get_mut(handle).source = PermissionEventSource::StateExit
                    }
                    _ => unreachable!(),
                }
                if let Ok(changed) = crate::settle_checked_execution(changed) {
                    // The public rebuild refreshes call plans; partial cleanup has
                    // its own existing producer and must also be rederived.
                    let rebuilt = crate::execution::terminal_unit::build_checked_partial_affine_unit_cleanup_plans(
                        &changed.typed,
                        &changed.facts,
                        &changed.facts.flow.terminal_unit_effects,
                    );
                    assert!(
                        rebuilt.for_machine(machine).is_none(),
                        "{mutation} {:?}, boundary={boundary}",
                        event.kind
                    );
                    assert!(
                        changed
                            .facts
                            .flow
                            .terminal_unit_effects
                            .for_machine(machine)
                            .is_none()
                    );
                }
            }
        }
    }
}
