use language_semantics::{Multiplicity, PermissionEventKind};

use super::{LIMITS, support};

#[test]
fn owned_graph_publication_rejoins_affine_discard_and_transfer_permissions() {
    let (original, _, _, _) = support::publish(LIMITS, "enter");
    for kind in [
        PermissionEventKind::AffineDrop,
        PermissionEventKind::Transfer,
    ] {
        let rows = original
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter_map(|(handle, event)| {
                (event.kind == kind && event.multiplicity == Multiplicity::Affine).then_some(handle)
            })
            .collect::<Vec<_>>();
        assert!(
            !rows.is_empty(),
            "{kind:?} is present in real source checking"
        );
        for handle in rows {
            for mutation in 0..4 {
                let mut changed = original.clone();
                let permissions = &mut changed.facts.flow.ownership.permissions;
                match mutation {
                    0 => permissions.get_mut(handle).machine_symbol = Default::default(),
                    1 => permissions.get_mut(handle).multiplicity = Multiplicity::Unrestricted,
                    2 => permissions.get_mut(handle).obligation_live = true,
                    _ => {
                        permissions.append(permissions.get(handle).clone());
                    }
                }
                support::reject(&changed, &format!("{kind:?} row mutation {mutation}"));
            }
        }
    }
}

#[test]
fn affine_permission_cannot_erase_a_selected_cleanup_dependency() {
    let (mut checked, _, _, _) = support::publish(LIMITS, "enter");
    let inspect = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap()
        .symbol;
    checked
        .facts
        .flow
        .semantic_dependencies
        .rows
        .push(checked_trees::CheckedSemanticDependency {
            consumer_machine: inspect,
            dependency: inspect,
            exposure: checked_trees::CheckedSemanticDependencyExposure::PrivateImplementation,
            kind: checked_trees::CheckedSemanticDependencyKind::AutomaticCleanupMachine,
        });
    support::reject(&checked, "no-code graph acquired executable cleanup");
}
