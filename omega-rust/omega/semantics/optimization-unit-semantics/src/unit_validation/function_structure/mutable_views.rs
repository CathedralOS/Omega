//! Current-graph exclusive bindings and fresh mutable view observations.
use super::*;
use terminal_psi::{ByteSequenceCarrier, StructuralAccess, StructuralTypeShape};

pub(super) fn validate(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    let mutable = function
        .structural_parameters
        .iter()
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .filter(|parameter| {
            parameter.access == StructuralAccess::MutableBorrow
                && types
                    .get(&parameter.structural_type)
                    .is_some_and(|declaration| {
                        matches!(
                            declaration.shape,
                            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView)
                        )
                    })
        })
        .map(|parameter| parameter.place)
        .collect::<BTreeSet<_>>();
    if mutable.is_empty() {
        return Ok(());
    }
    let entry = function
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .filter(|place| mutable.contains(place))
        .collect::<BTreeSet<_>>();
    let mut arrivals = function
        .blocks
        .iter()
        .map(|block| {
            (
                block.id,
                if block.id == function.entry {
                    entry.clone()
                } else {
                    mutable.clone()
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    // Start from the greatest candidate set and intersect every incoming path,
    // including backedges. A descriptor address does not keep a consumed loan live.
    loop {
        let previous = arrivals.clone();
        for block in &function.blocks {
            let mut incoming = if block.id == function.entry {
                entry.clone()
            } else {
                mutable.clone()
            };
            let mut reached = block.id == function.entry;
            for predecessor in &function.blocks {
                for edge in predecessor
                    .nodes
                    .iter()
                    .flat_map(|node| &node.successors)
                    .filter(|edge| edge.target == block.id)
                {
                    reached = true;
                    let mut carried = previous[&predecessor.id].clone();
                    for binding in &edge.structural_bindings {
                        if mutable.contains(&binding.argument.place) {
                            carried.remove(&binding.argument.place);
                        }
                    }
                    for parameter in &block.structural_parameters {
                        carried.remove(&parameter.place);
                    }
                    for binding in &edge.structural_bindings {
                        if mutable.contains(&binding.parameter) {
                            carried.insert(binding.parameter);
                        }
                    }
                    incoming.retain(|place| carried.contains(place));
                }
            }
            if !reached {
                incoming.clear();
            }
            arrivals.insert(block.id, incoming);
        }
        if arrivals == previous {
            break;
        }
    }
    for block in &function.blocks {
        let available = &arrivals[&block.id];
        for (position, node) in block.nodes.iter().enumerate() {
            let invalid = |place| OptimizationUnitValidationError::StructuralPlaceNotAvailable {
                machine: function.machine,
                block: block.id,
                node: position as u32,
                place,
            };
            let mut seen = BTreeSet::new();
            for place in super::structural_roots::operation_place_inputs(&node.operation) {
                if mutable.contains(&place)
                    && (!available.contains(&place)
                        || (node.successors.is_empty() && !seen.insert(place)))
                {
                    return Err(invalid(place));
                }
            }
            for edge in &node.successors {
                let mut seen = BTreeSet::new();
                for binding in &edge.structural_bindings {
                    let place = binding.argument.place;
                    if mutable.contains(&place) && !seen.insert(place) {
                        return Err(invalid(place));
                    }
                }
            }
            if let O::ByteSequenceWrite {
                destination,
                length,
                ..
            } = node.operation
            {
                // Rebound descriptors receive fresh observations at their current
                // block. Calls cannot preserve an old extent merely by preserving
                // the descriptor's address; a following measurement must rejoin it.
                let observed = block.nodes[..position].iter().rposition(|prior| matches!(prior.operation,
                    O::ByteSequenceLength { source, result, .. } if source == destination && result.value == length));
                if observed.is_none_or(|observed| {
                    block.nodes[observed + 1..position].iter().any(|prior| {
                        !super::structural_roots::operation_place_inputs(&prior.operation)
                            .is_empty()
                            && !matches!(
                                prior.operation,
                                O::ByteSequenceLength { .. } | O::ByteSequenceWrite { .. }
                            )
                    })
                }) {
                    return Err(invalid(destination));
                }
            }
        }
    }
    Ok(())
}
