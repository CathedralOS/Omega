use super::machine_and_entry_state;
use crate::tests::front_end::checked_program;

#[test]
fn owned_call_result_cleanup_requires_exact_transfer_on_every_edge() {
    use language_semantics::{PermissionEventKind, PermissionEventSource, PermissionProvenance};
    let checked = checked_program(
        r#"
        data Kind { case Missing; case Other; }
        machine make() -> Kind { Kind::Missing }
        machine route(choose: bool) {
            let kind: Kind = make();
            transition choose {
                true -> first(kind)
                false -> second(kind)
            }
            state first(kind: Kind) {}
            state second(kind: Kind) {}
        }
        "#,
    );
    let (machine, state) = machine_and_entry_state(&checked, "route");
    let rebuild = |facts: &checked_trees::CheckFacts| {
        crate::execution::terminal_cleanup::build_checked_structural_control_cleanup_plans(
            &checked.typed,
            facts,
        )
    };
    let plans = rebuild(&checked.facts);
    let plan = plans
        .for_state(machine, state)
        .expect("both edges transfer the owned result");
    assert_eq!(plan.edges.len(), 2);
    assert!(
        plan.edges
            .iter()
            .all(|edge| edge.trivial_affine_discard_parameter_positions.is_empty())
    );
    let transfers = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine
                && event.state_symbol == state
                && event.kind == PermissionEventKind::Transfer
        })
        .map(|(handle, event)| (handle, event.clone()))
        .collect::<Vec<_>>();
    assert_eq!(transfers.len(), 2);
    for (handle, original) in transfers {
        for corruption in [
            "missing edge",
            "duplicate move",
            "wrong origin",
            "wrong target",
            "live obligation",
            "stale path",
            "extra transfer",
        ] {
            let mut changed = checked.facts.clone();
            let permissions = &mut changed.flow.ownership.permissions;
            match corruption {
                "missing edge" => permissions.get_mut(handle).machine_symbol = Default::default(),
                "duplicate move" => {
                    permissions.insert(original.clone());
                }
                "wrong origin" => {
                    permissions.get_mut(handle).provenance = PermissionProvenance::Unknown
                }
                "wrong target" => {
                    permissions.get_mut(handle).source = PermissionEventSource::Call {
                        statement_index: match original.source {
                            PermissionEventSource::Call {
                                statement_index, ..
                            } => statement_index,
                            _ => panic!("transfer call"),
                        },
                        call_ordinal: 0,
                        target_symbol: machine,
                    }
                }
                "live obligation" => permissions.get_mut(handle).obligation_live = true,
                "stale path" => {
                    permissions.get_mut(handle).segments =
                        arena::HandleSpan::from_parts(arena::Handle::invalid(), 1)
                }
                "extra transfer" => {
                    let mut extra = original.clone();
                    extra.source = PermissionEventSource::Statement { statement_index: 0 };
                    permissions.insert(extra);
                }
                _ => unreachable!(),
            }
            assert!(
                rebuild(&changed).for_state(machine, state).is_none(),
                "{corruption} must not publish a partial cleanup plan"
            );
        }
    }
}
