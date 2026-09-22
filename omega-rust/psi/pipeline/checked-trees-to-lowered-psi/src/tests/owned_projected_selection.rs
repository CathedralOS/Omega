//! Owned match arms may move one projected affine child while each root's
//! residual siblings die on the actual selected edge.

use super::{CheckedTrees, checked_source, lower_machine};
use crate::TerminalMachineSelection;
use terminal_psi::Terminator;
fn projected_selection_source() -> CheckedTrees {
    checked_source(
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

fn choose_machine(lowered: &terminal_psi::TerminalModule) -> &terminal_psi::TerminalMachine {
    lowered
        .machines
        .iter()
        .find(|machine| {
            machine.blocks.iter().any(|block| {
                matches!(&block.terminator, Terminator::Jump { structural_arguments, .. }
                    if structural_arguments.iter().any(|argument| !argument.path.is_empty()))
            })
        })
        .expect("choose keeps its path-bearing successors")
}

fn projected_edges(
    machine: &terminal_psi::TerminalMachine,
) -> Vec<(
    &[terminal_psi::StructuralArgument],
    &[semantic_vocabulary::PlaceId],
    &[terminal_psi::StructuralAffineDiscard],
)> {
    machine
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Terminator::Jump {
                structural_arguments,
                trivial_affine_discards,
                residual_affine_discards,
                ..
            } if structural_arguments
                .iter()
                .any(|argument| !argument.path.is_empty()) =>
            {
                Some((
                    structural_arguments.as_slice(),
                    trivial_affine_discards.as_slice(),
                    residual_affine_discards.as_slice(),
                ))
            }
            _ => None,
        })
        .collect()
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
fn projected_owned_selection_lowers_path_arguments_and_residual_cleanup() {
    let checked = projected_selection_source();
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("choose"))
        .expect("projected selection lowers");
    let edges = projected_edges(choose_machine(&lowered.semantic_module));
    assert_eq!(edges.len(), 2, "one projected edge per selected arm");
    let field = |name: &str| vec![terminal_psi::StructuralPathSegment::from(name)];
    // `pair.first` moves; its untouched sibling `pair.second` dies on the edge.
    let local = edges
        .iter()
        .find(|(arguments, trivial, residuals)| {
            trivial.is_empty()
                && arguments
                    .iter()
                    .any(|argument| argument.path == field("first"))
                && residuals
                    .iter()
                    .any(|discard| discard.path == field("second"))
        })
        .expect("local projection edge");
    // `supply(...).second` moves the product's child; the product's own
    // residual `first` dies on the same edge while `pair`, the complement
    // roster source, is discarded whole.
    let product = edges
        .iter()
        .find(|(arguments, trivial, residuals)| {
            !trivial.is_empty()
                && arguments
                    .iter()
                    .any(|argument| argument.path == field("second"))
                && residuals
                    .iter()
                    .any(|discard| discard.path == field("first"))
        })
        .expect("product projection edge");
    for (arguments, _, residuals) in [local, product] {
        for argument in arguments
            .iter()
            .filter(|argument| !argument.path.is_empty())
        {
            assert!(
                residuals
                    .iter()
                    .all(|discard| discard.place == argument.place),
                "residual cleanup stays under the moved root"
            );
            assert_eq!(argument.access, terminal_psi::StructuralAccess::Owned);
        }
    }
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("projected selection terminal verifies");
    let bytes = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("projected selection terminal encodes");
    assert_eq!(
        terminal_codec::decode_module(&bytes).expect("canonical bytes decode"),
        lowered.semantic_module
    );
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

#[test]
fn projected_owned_selection_verifier_rejects_mutated_edges() {
    for mutation in 0..3 {
        let checked = projected_selection_source();
        let mut lowered =
            lower_machine(&checked, TerminalMachineSelection::Name("choose")).expect("lowers");
        for block in lowered
            .semantic_module
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
        {
            let Terminator::Jump {
                structural_arguments,
                residual_affine_discards,
                ..
            } = &mut block.terminator
            else {
                continue;
            };
            match mutation {
                // The moved path must equal the checked projection.
                0 => {
                    for argument in structural_arguments
                        .iter_mut()
                        .filter(|argument| !argument.path.is_empty())
                    {
                        argument.path[0] = terminal_psi::StructuralPathSegment::from("forged");
                    }
                }
                // Each recorded residual must be the exact complement.
                1 => {
                    for discard in residual_affine_discards.iter_mut() {
                        discard.path[0] = terminal_psi::StructuralPathSegment::from("forged");
                    }
                }
                // Dropping the residual leaves the sibling undisposed.
                _ => residual_affine_discards.clear(),
            }
        }
        assert!(
            terminal_verifier::validate_module(&lowered.semantic_module).is_err(),
            "mutation {mutation} must reject"
        );
    }
}
