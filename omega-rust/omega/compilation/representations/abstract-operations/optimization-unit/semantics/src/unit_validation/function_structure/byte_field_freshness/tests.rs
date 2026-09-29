//! Mutation controls isolate current-CFG freshness from source provenance checks.
use super::validate;
use crate::tests::indexed_byte_fields::indexed_field_unit;
use crate::tests::support::id;
use abstract_operations::AbstractOperation as O;
use abstract_operations::AbstractStructuralCallResult;
use optimization_unit::{OptimizationBlock, OptimizationEdge};
use semantic_vocabulary::{BlockId, OperationId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::StructuralAccess;

#[test]
fn primitive_writes_invalidate_only_potentially_overlapping_projections() {
    use crate::tests::indexed_byte_fields::indexed_field_with_primitive_sibling_unit;
    use terminal_psi::StructuralPathSegment as S;

    let unit = indexed_field_with_primitive_sibling_unit();
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
    let primitive = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find(|node| matches!(node.operation, O::WriteOnlyPrimitiveStore { .. }))
        .unwrap();
    let O::WriteOnlyPrimitiveStore {
        destination,
        path: written,
        ..
    } = &primitive.operation
    else {
        unreachable!();
    };
    assert!(matches!(
        written.as_slice(),
        [S::Field(_), S::RuntimeIndex { .. }]
    ));
    let root = destination.place;
    let field = written[0].clone();
    let other_runtime = S::RuntimeIndex {
        index: id(991, semantic_vocabulary::ValueId::new),
        obligation: id(992, semantic_vocabulary::ObligationId::new),
    };
    // These isolate mutation classification; complete operation typing and
    // runtime-index obligation checks remain the unit validator's job.
    for (observed, invalidated) in [
        (vec![S::Field("out".into())], false),
        (vec![], true),
        (vec![field.clone()], true),
        (written.clone(), true),
        (vec![field.clone(), S::FixedIndex(0)], true),
        (vec![field, other_runtime], true),
    ] {
        assert_eq!(
            super::changes_length(function, &types, root, &observed, &primitive.operation),
            invalidated,
            "observed projection: {observed:?}"
        );
    }
    let mut whole_root = primitive.operation.clone();
    let O::WriteOnlyPrimitiveStore { path, .. } = &mut whole_root else {
        unreachable!()
    };
    path.clear();
    assert!(super::changes_length(
        function,
        &types,
        root,
        &[S::Field("out".into())],
        &whole_root
    ));
    assert!(!super::changes_length(
        function,
        &types,
        id(999, semantic_vocabulary::PlaceId::new),
        &[],
        &whole_root
    ));
}

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
        call.operation = O::CallStructural {
            psi_operation: id(991, OperationId::new),
            callee: function.machine,
            arguments: Vec::new(),
            structural_arguments: vec![terminal_psi::StructuralArgument {
                place: destination,
                access,
                path: Vec::new(),
            }],
            dynamic_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
            result: AbstractStructuralCallResult::Unit,
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
