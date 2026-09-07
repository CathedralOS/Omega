//! Shared temporary access retains one real owner until the call continuation.

use super::*;
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionEventKind, PermissionEventSource,
    PermissionProvenance,
};

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
fn anonymous_shared_permissions_retain_then_discard_exact_expression_owner() {
    for boundary in [false, true] {
        let checked = source(boundary, "");
        let machine = machine_named(&checked, "main");
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine)
            .expect("anonymous shared Unit plan");
        assert_eq!(plan.operations.len(), 4);
        let (producer_coordinate, producer_state) = match &plan.operations[0] {
            CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate,
                target_state,
                result,
                discard_result_on_return,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                coordinate,
                target_state,
                result,
                discard_result_on_return,
                ..
            } => {
                assert!(!*discard_result_on_return);
                assert_eq!((result.statement_index, result.binding_ordinal), (0, 0));
                (*coordinate, *target_state)
            }
            _ => panic!("real structural producer"),
        };
        assert_eq!(
            (
                producer_coordinate.statement_index,
                producer_coordinate.call_ordinal
            ),
            (0, 1)
        );
        let CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            structural_arguments,
            target_state,
            claim_transfers,
            ..
        } = &plan.operations[1]
        else {
            panic!("ordinary shared consumer");
        };
        assert_eq!(
            (coordinate.statement_index, coordinate.call_ordinal),
            (0, 0)
        );
        assert!(claim_transfers.is_empty());
        let [argument] = structural_arguments.as_slice() else {
            panic!("one shared argument");
        };
        assert_eq!(
            argument.source,
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: 0
            }
        );
        assert_eq!(
            argument.access,
            checked_trees::CheckedStructuralAccess::SharedBorrow
        );
        assert!(argument.path.is_empty());
        let events = checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .map(|(_, event)| event)
            .filter(|event| {
                event.machine_symbol == machine
                    && matches!(event.root, facts::PlaceRoot::Expression(_))
            })
            .collect::<Vec<_>>();
        let [establish, borrow, discard] = events.as_slice() else {
            panic!("owner, shared loan, and dying owner");
        };
        let producer_source = PermissionEventSource::Call {
            statement_index: 0,
            call_ordinal: 1,
            target_symbol: producer_state,
        };
        let consumer_source = PermissionEventSource::Call {
            statement_index: 0,
            call_ordinal: 0,
            target_symbol: *target_state,
        };
        let provenance = PermissionProvenance::Established {
            machine_symbol: machine,
            state_symbol: plan.state,
            source: producer_source,
        };
        assert_eq!(
            (
                establish.kind,
                establish.access,
                establish.multiplicity,
                establish.source
            ),
            (
                PermissionEventKind::Establish,
                PermissionAccess::Owned,
                Multiplicity::Affine,
                producer_source
            )
        );
        assert_eq!(
            (
                borrow.kind,
                borrow.access,
                borrow.multiplicity,
                borrow.source
            ),
            (
                PermissionEventKind::Establish,
                PermissionAccess::Shared,
                Multiplicity::Unrestricted,
                consumer_source
            )
        );
        assert_eq!(
            (
                discard.kind,
                discard.access,
                discard.multiplicity,
                discard.source
            ),
            (
                PermissionEventKind::AffineDrop,
                PermissionAccess::Owned,
                Multiplicity::Affine,
                consumer_source
            )
        );
        for event in &events {
            assert_eq!(event.root, establish.root);
            assert_eq!(event.provenance, provenance);
            assert_eq!(event.state_symbol, plan.state);
            assert!(!event.obligation_live);
            assert!(
                checked
                    .facts
                    .flow
                    .ownership
                    .segments
                    .span(event.segments)
                    .unwrap()
                    .is_empty()
            );
        }
        let input_transfers = checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter(|(_, event)| {
                event.machine_symbol == machine
                    && event.kind == PermissionEventKind::Transfer
                    && event.access == PermissionAccess::Owned
            })
            .count();
        assert_eq!(input_transfers, usize::from(!boundary));
    }
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
                if crate::rebuild_checked_terminal_plans_with_selected_execution(
                    &mut changed,
                    &[],
                    &[],
                )
                .is_ok()
                {
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
