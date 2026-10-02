//! Shared child observations freeze their exact parent region until the final use.

use super::address_joins::{STATE_RECORD_VIEW_SOURCE, validate_address_join_ownership};
use crate::tests::structural_cases::verified_source_machine_unit;
use crate::tests::support::refresh_node_derivatives;
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use abstract_operations::AbstractOperation as O;

fn current_child_source() -> String {
    STATE_RECORD_VIEW_SOURCE.replace(
        "view.len == 4 && packet.items[0] == 70 && packet.items[3] == 5 && packet.marker == 0",
        "view.len == 4",
    )
}

#[test]
fn address_join_fresh_child_observation_keeps_current_parent_authority() {
    let candidate = verified_source_machine_unit(&current_child_source(), "main");
    validate_psi_optimization_unit(&candidate)
        .expect("the fresh shared child belongs to the current exclusive arrival");
}

#[test]
fn address_join_shared_child_freezes_until_the_last_observation() {
    let original = verified_source_machine_unit(&current_child_source(), "main");
    validate_psi_optimization_unit(&original).unwrap();
    for after_observation in [false, true] {
        let mut candidate = original.clone();
        let (block, parent) = candidate.functions[0]
            .blocks
            .iter()
            .enumerate()
            .find_map(|(position, block)| {
                block.nodes.iter().find_map(|node| match &node.operation {
                    O::EstablishElementView { source, .. } => Some((position, source.place)),
                    _ => None,
                })
            })
            .unwrap();
        let nodes = &mut candidate.functions[0].blocks[block].nodes;
        let call = nodes
            .iter()
            .position(|node| {
                matches!(&node.operation,
                    O::CallStructural { structural_arguments, .. }
                    if structural_arguments.iter().any(|argument| argument.place == parent)
                )
            })
            .expect("stamp mutates the same parent array");
        let stamp = nodes.remove(call);
        let insertion = nodes
            .iter()
            .position(|node| {
                if after_observation {
                    matches!(node.operation, O::ByteSequenceLength { .. })
                } else {
                    matches!(node.operation, O::EstablishElementView { .. })
                }
            })
            .unwrap()
            + 1;
        nodes.insert(insertion, stamp);
        for position in 0..nodes.len() {
            refresh_node_derivatives(&mut candidate, 0, block, position);
        }
        let result = validate_address_join_ownership(&candidate);
        if after_observation {
            result.expect("the last scalar observation ends this nonescaping cohort");
        } else {
            assert!(
                matches!(result,
                    Err(OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed { place, .. })
                    if place == parent
                ),
                "the child still needs its parent's array: {result:?}"
            );
        }
    }
}

#[test]
fn address_join_shared_child_does_not_freeze_a_sibling_field() {
    let source = current_child_source().replace(
        "let view: &[u8] = packet.items.as_slice();",
        "let view: &[u8] = packet.items.as_slice(); packet.marker = 9;",
    );
    let candidate = verified_source_machine_unit(&source, "main");
    validate_psi_optimization_unit(&candidate)
        .expect("the child captures items, while marker is a disjoint sibling");
}

#[test]
fn address_join_read_only_call_does_not_escape_child() {
    let source = format!(
        "machine observed_length(values: &[u8]) -> u64 {{ transition {{ _ -> (values.len) }} }}\n{}",
        current_child_source().replace(
            "transition view.len == 4",
            "let length: u64 = observed_length(view); transition length == 4"
        )
    );
    let candidate = verified_source_machine_unit(&source, "main");
    validate_psi_optimization_unit(&candidate)
        .expect("the completed read-only call leaves no escaping descriptor at owner cleanup");
}

fn closed_call_candidate(writing_callee: bool) -> optimization_unit::PsiOptimizationUnit {
    let source = format!(
        "machine observe(values: &mut [u8]) {{ }}\n{}",
        current_child_source().replace(
            "stamp(&mut packet.items);",
            "observe(&mut packet.items); stamp(&mut packet.items);"
        )
    );
    let mut candidate = verified_source_machine_unit(&source, "main");
    validate_psi_optimization_unit(&candidate).unwrap();
    let block = candidate.functions[0]
        .blocks
        .iter()
        .position(|block| {
            block
                .nodes
                .iter()
                .any(|node| matches!(node.operation, O::EstablishElementView { .. }))
        })
        .unwrap();
    let nodes = &mut candidate.functions[0].blocks[block].nodes;
    let calls = nodes
        .iter()
        .enumerate()
        .filter_map(|(position, node)| match node.operation {
            O::CallStructural { callee, .. } => Some((position, callee)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    assert_ne!(calls[0].1, calls[1].1);
    let mut observation = nodes.remove(calls[0].0);
    if writing_callee && let O::CallStructural { callee, .. } = &mut observation.operation {
        *callee = calls[1].1;
    }
    let formation = nodes
        .iter()
        .position(|node| matches!(node.operation, O::EstablishElementView { .. }))
        .unwrap();
    nodes.insert(formation + 1, observation);
    for position in 0..nodes.len() {
        refresh_node_derivatives(&mut candidate, 0, block, position);
    }
    candidate
}

#[test]
fn closed_call_does_not_disturb_an_unwritten_mutable_input() {
    validate_address_join_ownership(&closed_call_candidate(false))
        .expect("the current ordinary callee has no write through its mutable formal");
}

#[test]
fn closed_call_uses_the_current_callee_writes() {
    assert!(
        matches!(
            validate_address_join_ownership(&closed_call_candidate(true)),
            Err(OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed { .. })
        ),
        "a rewritten same-signature callee writes the live child's backing"
    );
}
