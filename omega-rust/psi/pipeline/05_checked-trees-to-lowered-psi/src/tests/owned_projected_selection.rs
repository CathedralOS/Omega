//! Owned match arms may move one projected affine child while each root's
//! residual siblings die on the actual selected edge.

use super::{CheckedTrees, lower_machine};
use crate::TerminalMachineSelection;
fn projected_selection_source() -> CheckedTrees {
    crate::front_end::checked_program(
        "data Payload { left: u64; right: u64; }
         data Pair { first: Payload; second: Payload; }
         machine supply(first: u64, second: u64) -> Pair {
             Pair { first: Payload { left: first, right: second }, second: Payload { left: second, right: first } }
         }
         machine choose(selected: u64, first: u64, second: u64) -> u64 {
             let pair: Pair = Pair { first: Payload { left: first, right: second }, second: Payload { left: second, right: first } };
             let result: Payload = match selected { 0 -> pair.first, _ -> supply(first, second).second };
             result.left ^ result.right
         }",
    )
}

fn projection_handles(checked: &CheckedTrees) -> Vec<checked_trees::CheckedStructuralValueHandle> {
    checked
        .facts
        .values
        .structural_values
        .nodes
        .iter()
        .filter_map(|(handle, node)| {
            matches!(
                node.kind,
                checked_trees::CheckedStructuralValueKind::Projection { .. }
            )
            .then_some(handle)
        })
        .collect()
}

#[test]
fn projected_owned_selection_rejects_mutated_projection_evidence() {
    for mutation in 0..4 {
        let mut checked = projected_selection_source();
        match mutation {
            // The retained node path must equal the replayed authored chain.
            0 => {
                let handles = projection_handles(&checked);
                assert_eq!(handles.len(), 2);
                for handle in handles {
                    let node = checked.facts.values.structural_values.nodes.get_mut(handle);
                    let checked_trees::CheckedStructuralValueKind::Projection { path, .. } =
                        &mut node.kind
                    else {
                        unreachable!()
                    };
                    path[0] =
                        checked_trees::CheckedUnitStructuralPathSegment::Field("second".into());
                }
            }
            // The leaf identity must equal the receipt's result type.
            1 => {
                for handle in projection_handles(&checked) {
                    let node = checked.facts.values.structural_values.nodes.get_mut(handle);
                    let checked_trees::CheckedStructuralValueKind::Projection {
                        type_identity, ..
                    } = &mut node.kind
                    else {
                        unreachable!()
                    };
                    *type_identity = "forged".to_owned();
                }
            }
            // The recorded transfer path must equal the authored place chain.
            2 => {
                let span = {
                    let ownership = &checked.facts.flow.ownership;
                    ownership
                        .selection_transfers
                        .iter()
                        .map(|(_, transfer)| transfer.path)
                        .next()
                        .expect("transfer path")
                };
                *checked.facts.flow.ownership.segments.get_mut(span.start()) =
                    facts::PlaceSegment::FixedIndex { index: 0 };
            }
            // A call-product transfer must not gain a roster source.
            _ => {
                let ownership = &mut checked.facts.flow.ownership;
                let source = ownership.selection_sources.iter().next().expect("source").0;
                let handle = ownership
                    .selection_transfers
                    .iter()
                    .find(|(_, transfer)| !transfer.source.is_valid())
                    .expect("product transfer")
                    .0;
                ownership.selection_transfers.get_mut(handle).source = source;
            }
        }
        assert!(
            lower_machine(&checked, TerminalMachineSelection::Name("choose")).is_err(),
            "mutation {mutation} must reject"
        );
    }
}
