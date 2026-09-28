//! Mutation controls isolate current-CFG freshness from source provenance checks.
use super::validate;
use crate::tests::indexed_byte_fields::indexed_field_unit;
use crate::tests::support::id;
use abstract_operations::AbstractOperation as O;
use optimization_unit::{OptimizationBlock, OptimizationEdge};
use semantic_vocabulary::{BlockId, OperationId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::StructuralAccess;

#[test]
fn indexed_byte_field_freshness_preserves_only_length_preserving_effects() {
    let unit = indexed_field_unit();
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == unit.entry)
        .unwrap();
    let types = unit
        .structural_types
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();
    let block = function
        .blocks
        .iter()
        .position(|block| {
            block.nodes.iter().any(|node| {
                matches!(
                    node.operation,
                    O::StructuralByteSequenceFieldByteStore { .. }
                )
            })
        })
        .unwrap();
    let store = function.blocks[block]
        .nodes
        .iter()
        .position(|node| {
            matches!(
                node.operation,
                O::StructuralByteSequenceFieldByteStore { .. }
            )
        })
        .unwrap();
    let mut replacements = function.blocks[block]
        .nodes
        .iter()
        .filter(|node| matches!(node.operation, O::StructuralByteSequenceFieldStore { .. }));
    let overlapping = replacements.next().unwrap().clone();
    let sibling = replacements.next().unwrap().clone();
    let indexed = function.blocks[block].nodes[store].clone();
    let O::StructuralByteSequenceFieldByteStore { destination, .. } = indexed.operation else {
        unreachable!();
    };
    for (intervening, accepted) in [
        (overlapping, false),
        (sibling, true),
        (indexed.clone(), true),
    ] {
        let mut changed = function.clone();
        changed.blocks[block].nodes.insert(store, intervening);
        assert_eq!(
            validate(&changed, &types, &BTreeMap::new()).is_ok(),
            accepted
        );
    }
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
        StructuralAccess::Owned,
    ] {
        let mut changed = function.clone();
        let mut call = indexed.clone();
        call.operation = O::CallUnit {
            psi_operation: id(991, OperationId::new),
            callee: function.machine,
            arguments: Vec::new(),
            structural_arguments: vec![terminal_psi::StructuralArgument {
                place: destination,
                access,
                path: Vec::new(),
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        };
        changed.blocks[block].nodes.insert(store, call);
        assert_eq!(
            validate(&changed, &types, &BTreeMap::new()).is_ok(),
            access == StructuralAccess::SharedBorrow
        );
    }
    let mut changed = function.clone();
    let mut unknown = indexed;
    unknown.operation = O::PortWrite {
        psi_operation: id(992, OperationId::new),
        service: id(993, semantic_vocabulary::ServiceId::new),
        port: 0,
        value: 0,
    };
    changed.blocks[block].nodes.insert(store, unknown);
    assert!(validate(&changed, &types, &BTreeMap::new()).is_err());
}

#[test]
fn indexed_byte_field_length_freshness_examines_backedges_and_missing_arrivals() {
    let unit = indexed_field_unit();
    let mut function = unit
        .functions
        .iter()
        .find(|function| function.machine == unit.entry)
        .unwrap()
        .clone();
    let types = unit
        .structural_types
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();
    let entry = function
        .blocks
        .iter()
        .position(|block| block.id == function.entry)
        .unwrap();
    let store = function.blocks[entry]
        .nodes
        .iter()
        .position(|node| {
            matches!(
                node.operation,
                O::StructuralByteSequenceFieldByteStore { .. }
            )
        })
        .unwrap();
    let replacement = function.blocks[entry]
        .nodes
        .iter()
        .find(|node| matches!(node.operation, O::StructuralByteSequenceFieldStore { .. }))
        .unwrap()
        .clone();
    let indexed = function.blocks[entry].nodes[store].clone();
    let loop_block = id(994, BlockId::new);
    let edge = |identity| OptimizationEdge {
        psi_edge: id(identity, semantic_vocabulary::EdgeId::new),
        target: loop_block,
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
        provenance: Vec::new(),
        fuel: Vec::new(),
    };
    // Keep the exact source observation in entry and put the indexed write
    // behind both entry and a cyclic arrival. These tests exercise the freshness
    // algorithm directly; complete CFG/provenance validation has separate owners.
    function.blocks[entry].nodes.truncate(store);
    function.blocks[entry].nodes.last_mut().unwrap().successors = vec![edge(995)];
    let mut backedge = indexed.clone();
    backedge.successors = vec![edge(996)];
    function.blocks.push(OptimizationBlock {
        id: loop_block,
        structural_parameters: Vec::new(),
        parameters: Vec::new(),
        nodes: vec![indexed, backedge],
    });
    let predecessors = BTreeMap::from([(loop_block, BTreeSet::from([function.entry, loop_block]))]);
    validate(&function, &types, &predecessors).unwrap();
    let loop_position = function.blocks.len() - 1;
    function.blocks[loop_position].nodes[1].operation = replacement.operation;
    assert!(validate(&function, &types, &predecessors).is_err());
    let missing = BTreeMap::from([(loop_block, BTreeSet::new())]);
    assert!(validate(&function, &types, &missing).is_err());
}
