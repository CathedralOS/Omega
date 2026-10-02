use super::machine_and_entry_state;
use crate::tests::front_end::checked_program;

#[test]
fn nested_affine_local_construction_retains_exact_state_cleanup() {
    let checked = checked_program(
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
    let (machine, state) = machine_and_entry_state(&checked, "route");
    let permissions = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| event.machine_symbol == machine && event.state_symbol == state)
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    let plan = checked
        .facts
        .flow
        .terminal_structural_control_cleanups
        .for_state(machine, state)
        .unwrap_or_else(|| {
            panic!("completed parent requires ordinary edge custody: {permissions:#?}")
        });
    assert_eq!(plan.edges.len(), 1);
    assert!(
        plan.edges[0]
            .trivial_affine_discard_parameter_positions
            .is_empty()
    );
}

#[test]
fn nested_affine_local_construction_requires_exact_establishment() {
    use language_semantics::{PermissionEventKind, PermissionEventSource, PermissionProvenance};
    let checked = checked_program(
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
    let (machine, state) = machine_and_entry_state(&checked, "route");
    let (handle, original) = checked
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
        .expect("parent establishment at its own declaration");
    assert_eq!(
        original.provenance,
        PermissionProvenance::Established {
            machine_symbol: machine,
            state_symbol: state,
            source: PermissionEventSource::Statement { statement_index: 0 },
        }
    );
    for corruption in ["missing", "duplicate", "displaced", "wrong provenance"] {
        let mut changed = checked.facts.clone();
        let permissions = &mut changed.flow.ownership.permissions;
        match corruption {
            "missing" => permissions.get_mut(handle).machine_symbol = Default::default(),
            "duplicate" => {
                permissions.insert(original.clone());
            }
            "displaced" => {
                permissions.get_mut(handle).source =
                    PermissionEventSource::Statement { statement_index: 0 }
            }
            "wrong provenance" => {
                permissions.get_mut(handle).provenance = PermissionProvenance::Unknown
            }
            _ => unreachable!(),
        }
        let plans =
            crate::execution::terminal_cleanup::build_checked_structural_control_cleanup_plans(
                &checked.typed,
                &changed,
            );
        assert!(
            plans.for_state(machine, state).is_none(),
            "{corruption} establishment must reject cleanup"
        );
    }
}
