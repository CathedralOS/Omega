use super::admission;
use language_semantics::{PermissionEventKind, PermissionEventSource, PermissionProvenance};

#[test]
fn nested_affine_local_construction_replays_conserved_provenance() {
    let checked = crate::front_end::checked_program(
        r#"
        data Item { value: i32; }
        data Holder { item: Item; }
        machine route() {
            let item: Item = Item { value: 70 };
            let holder: Holder = Holder { item: item };
            transition { _ -> finish(holder) }
            state finish(holder: Holder) {}
        }
        "#,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "route")
        .expect("source machine")
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .expect("constructed parent state graph");
    admission::admit(&checked, plan).expect("named child origin survives parent construction");
    let state = plan.states[0].state;
    let (establishment, original) = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .find(|(_, event)| {
            event.machine_symbol == machine
                && event.state_symbol == state
                && event.kind == PermissionEventKind::Establish
                && event.source == PermissionEventSource::Statement { statement_index: 1 }
        })
        .expect("parent establishment");
    for corruption in ["missing", "duplicate", "displaced", "rewritten origin"] {
        let mut changed = checked.clone();
        let permissions = &mut changed.facts.flow.ownership.permissions;
        match corruption {
            "missing" => permissions.get_mut(establishment).machine_symbol = Default::default(),
            "duplicate" => {
                permissions.insert(original.clone());
            }
            "displaced" => {
                permissions.get_mut(establishment).source =
                    PermissionEventSource::Statement { statement_index: 0 }
            }
            "rewritten origin" => {
                // Consistently forging every lifecycle row must still fail:
                // the initializer, rather than row agreement, owns origin.
                let handles = permissions
                    .iter()
                    .filter(|(_, event)| {
                        event.machine_symbol == machine
                            && event.state_symbol == state
                            && event.root == original.root
                    })
                    .map(|(handle, _)| handle)
                    .collect::<Vec<_>>();
                for handle in handles {
                    permissions.get_mut(handle).provenance = PermissionProvenance::Established {
                        machine_symbol: machine,
                        state_symbol: state,
                        source: PermissionEventSource::Statement { statement_index: 1 },
                    };
                }
            }
            _ => unreachable!(),
        }
        assert!(
            admission::admit(&changed, plan).is_err(),
            "{corruption} custody must reject source replay"
        );
    }
    let (transfer, original) = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .find(|(_, event)| {
            event.machine_symbol == machine
                && event.state_symbol == state
                && event.kind == PermissionEventKind::Transfer
                && event.source == PermissionEventSource::Statement { statement_index: 1 }
        })
        .expect("constructor consumes its named child at the declaration");
    for corruption in [
        "missing child move",
        "duplicate child move",
        "displaced child move",
        "disposed moved child",
    ] {
        let mut changed = checked.clone();
        let permissions = &mut changed.facts.flow.ownership.permissions;
        match corruption {
            "missing child move" => {
                permissions.get_mut(transfer).machine_symbol = Default::default()
            }
            "duplicate child move" => {
                permissions.insert(original.clone());
            }
            "displaced child move" => {
                permissions.get_mut(transfer).source =
                    PermissionEventSource::Statement { statement_index: 0 }
            }
            "disposed moved child" => {
                let mut disposal = original.clone();
                disposal.kind = PermissionEventKind::AffineDrop;
                disposal.source = PermissionEventSource::StateExit;
                permissions.insert(disposal);
            }
            _ => unreachable!(),
        }
        assert!(
            admission::admit(&changed, plan).is_err(),
            "{corruption} must reject exact operand retirement"
        );
    }
}
