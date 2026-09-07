//! Temporary projections retain exact producer, transfer, and residual facts.

use super::*;
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, PermissionProvenance,
};

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
fn anonymous_projection_permissions_name_exact_producer_and_residual() {
    for boundary in [false, true] {
        let checked = source(boundary);
        let machine = machine_named(&checked, "enter");
        let plan = checked
            .facts
            .flow
            .terminal_partial_affine_unit_cleanups
            .for_machine(machine)
            .expect("anonymous partial cleanup plan");
        let state = plan.machine.state;
        let flow = checked
            .facts
            .flow
            .control
            .states
            .iter()
            .map(|(_, flow)| flow)
            .find(|flow| flow.machine_symbol == machine && flow.state_symbol == state)
            .unwrap();
        let calls = checked.facts.flow.control.calls.span_or_empty(flow.calls);
        let producer = calls.iter().find(|call| call.call_ordinal == 1).unwrap();
        let consumer = calls.iter().find(|call| call.call_ordinal == 0).unwrap();
        assert_eq!((producer.statement_index, consumer.statement_index), (0, 0));
        let root = facts::PlaceRoot::Expression(producer.authored_expression);
        let rows = checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .map(|(_, event)| event)
            .filter(|event| event.machine_symbol == machine && event.root == root)
            .collect::<Vec<_>>();
        let [establish, transfer, discard] = rows.as_slice() else {
            panic!("three exact temporary events");
        };
        let producer_source = PermissionEventSource::Call {
            statement_index: 0,
            call_ordinal: 1,
            target_symbol: producer.target_symbol,
        };
        let consumer_source = PermissionEventSource::Call {
            statement_index: 0,
            call_ordinal: 0,
            target_symbol: consumer.target_symbol,
        };
        assert_eq!(
            (establish.kind, establish.source),
            (PermissionEventKind::Establish, producer_source)
        );
        assert_eq!(
            (transfer.kind, transfer.source),
            (PermissionEventKind::Transfer, consumer_source)
        );
        assert_eq!(
            (discard.kind, discard.source),
            (PermissionEventKind::AffineDrop, consumer_source)
        );
        let provenance = PermissionProvenance::Established {
            machine_symbol: machine,
            state_symbol: state,
            source: producer_source,
        };
        for event in &rows {
            assert_eq!(event.provenance, provenance);
            assert_eq!(event.state_symbol, state);
            assert_eq!(event.access, PermissionAccess::Owned);
            assert_eq!(event.multiplicity, Multiplicity::Affine);
            assert_eq!(event.claim_identity, PermissionClaimIdentity::Unknown);
            assert!(!event.obligation_live);
        }
        assert!(establish.segments.is_empty());
        for (event, name) in [(transfer, "right"), (discard, "left")] {
            let [facts::PlaceSegment::Field { symbol }] = checked
                .facts
                .flow
                .ownership
                .segments
                .span_or_empty(event.segments)
            else {
                panic!("one exact field");
            };
            assert!(checked.typed.data_definitions().iter().flat_map(|data| checked.typed.data_members(data))
                .any(|member| matches!(member, typed_trees::data::DataMember::Field(field) if field.symbol == *symbol && field.name.as_str() == name)));
        }
        assert_eq!(plan.machine.operations.len(), 3);
        assert!(matches!(
            plan.machine.operations.last(),
            Some(CheckedUnitEffectOperationPlan::ReturnUnit {
                statement_index: 1,
                ..
            })
        ));
        assert_eq!(plan.residual_affine_discards.len(), 1);
        assert_eq!(
            plan.residual_affine_discards[0].source,
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: 0
            }
        );
    }
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
                if crate::rebuild_checked_terminal_plans_with_selected_execution(
                    &mut changed,
                    &[],
                    &[],
                )
                .is_ok()
                {
                    // The public rebuild refreshes call plans; partial cleanup has
                    // its own existing producer and must also be rederived.
                    let rebuilt = crate::flow::build_checked_partial_affine_unit_cleanup_plans(
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
