//! Isolate current-owner replay using the operations of a source-produced
//! local snapshot. The full unit's positive validation lives beside its builder;
//! the frontier corruptions here are not claims of valid standalone programs.
use super::*;
use crate::tests::indexed_byte_fields::local_byte_field_snapshot_unit;
use crate::tests::support::id;
use crate::unit_validation::references::{
    LiveReference, ReferenceIdentity, ReferenceOrigin, ReferenceParent,
};
use semantic_vocabulary::PlaceId;
use terminal_psi::StructuralPathSegment;

#[test]
fn local_byte_observations_reject_consumed_partial_and_suspended_owners() {
    let unit = local_byte_field_snapshot_unit();
    let mut function = unit
        .functions
        .iter()
        .find(|function| function.machine == unit.entry)
        .unwrap()
        .clone();
    let root = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            O::EstablishZeroedStructural { result, .. } => Some(result.place),
            _ => None,
        })
        .expect("source establishes local record storage");
    let mut observations = Vec::new();
    for block in &function.blocks {
        for (node_position, node) in block.nodes.iter().enumerate() {
            if matches!(&node.operation, O::StructuralByteSequenceFieldLength { source, .. }
                if *source == root)
                || matches!(&node.operation, O::EstablishByteSequenceConcatenation { left, right, .. }
                    if left.source() == root || right.source() == root)
            {
                observations.push((block.id, node_position as u32, node.operation.clone()));
                if let O::EstablishByteSequenceConcatenation { left, right, .. } = &node.operation {
                    let mut reversed = node.operation.clone();
                    if let O::EstablishByteSequenceConcatenation {
                        left: reversed_left,
                        right: reversed_right,
                        ..
                    } = &mut reversed
                    {
                        *reversed_left = right.clone();
                        *reversed_right = left.clone();
                    }
                    observations.push((block.id, node_position as u32, reversed));
                }
            }
        }
    }
    assert!(
        observations.len() >= 3,
        "length and both concat operand positions"
    );
    for multiplicity in [
        StructuralMultiplicity::Unrestricted,
        StructuralMultiplicity::Affine,
    ] {
        // Vary only the contract consumed by ownership replay, not the source
        // catalog. This isolates the same frontier rule for copy and affine roots.
        for node in function
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.nodes)
        {
            if let O::EstablishZeroedStructural { result, .. } = &mut node.operation
                && result.place == root
            {
                result.multiplicity = multiplicity;
            }
        }
        let mut frontier = CurrentOwnership {
            claims: BTreeMap::new(),
            owned_places: BTreeMap::new(),
            partial_custody_paths: BTreeMap::new(),
            live_references: Vec::new(),
        };
        if multiplicity != StructuralMultiplicity::Unrestricted {
            frontier.owned_places.insert(root, multiplicity);
        }
        for (block, node, operation) in &observations {
            validate_byte_observations(&function, *block, *node, operation, &frontier).unwrap();
            let mut incomplete = frontier.clone();
            incomplete.partial_custody_paths.insert(
                root,
                BTreeSet::from([vec![StructuralPathSegment::Field("out".into())]]),
            );
            assert!(matches!(
                validate_byte_observations(&function, *block, *node, operation, &incomplete),
                Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive { place, .. }) if place == root
            ));
            if multiplicity == StructuralMultiplicity::Affine {
                let mut consumed = frontier.clone();
                consumed.owned_places.remove(&root);
                assert!(matches!(
                    validate_byte_observations(&function, *block, *node, operation, &consumed),
                    Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive { place, .. }) if place == root
                ));
            }
            let carrier = id(9001, PlaceId::new);
            let origin = ReferenceOrigin::Primitive(root);
            let mut live = vec![LiveReference {
                identity: ReferenceIdentity {
                    place: carrier,
                    path: Vec::new(),
                },
                carrier,
                carrier_path: Vec::new(),
                root: origin.clone(),
                parent: ReferenceParent::Root(origin),
            }];
            assert!(matches!(
                crate::current_ownership::references::apply_operation(
                    &function,
                    &BTreeMap::new(),
                    &BTreeMap::new(),
                    *block,
                    *node,
                    operation,
                    &mut live,
                ),
                Err(OptimizationUnitValidationError::CurrentReferenceCustodyViolation { .. })
            ));
            live.clear();
            crate::current_ownership::references::apply_operation(
                &function,
                &BTreeMap::new(),
                &BTreeMap::new(),
                *block,
                *node,
                operation,
                &mut live,
            )
            .unwrap();
        }
    }
}
