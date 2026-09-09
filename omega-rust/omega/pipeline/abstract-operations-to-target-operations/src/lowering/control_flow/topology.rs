//! Select graph lowering from actual topology, never from a ranking annotation.
use super::*;

pub(in crate::lowering) fn requires_graph(
    function: &AbstractFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<bool, LoweringError> {
    if function.operations.iter().any(|operation| {
        matches!(
            operation,
            AbstractOperation::EstablishPrimitiveLocal { .. }
                | AbstractOperation::EstablishScalarCase { .. }
                | AbstractOperation::PrimitiveLocalStore { .. }
                | AbstractOperation::PrimitiveScalarRead { .. }
        )
    }) {
        return Ok(true);
    }
    if function.operations.iter().any(|operation| {
        matches!(operation, AbstractOperation::CallStructural { result, .. }
            if matches!(types.get(&result.structural_type).map(|declaration| &declaration.shape),
                Some(StructuralTypeShape::Sum { .. })))
    }) {
        return Ok(true);
    }
    Ok(function.result.scalar().is_some()
        && (has_cycle(function)?
            || crate::lowering::unobserved_owned::has_block_arrivals(function, types)
            || function.operations.iter().any(|operation| {
                matches!(operation, AbstractOperation::WriteOnlyPrimitiveStore { .. })
            })))
}

fn has_cycle(function: &AbstractFunction) -> Result<bool, LoweringError> {
    let invalid = || LoweringError::ConditionalControlFlowRequiresBlockLowering(function.machine);
    let mut outgoing = BTreeMap::new();
    let mut incoming = BTreeMap::new();
    for (position, block) in function.block_entries.iter().enumerate() {
        if incoming.insert(block.block, 0_usize).is_some() {
            return Err(invalid());
        }
        let end = function
            .block_entries
            .get(position + 1)
            .map_or(function.operations.len(), |next| next.operation_offset);
        let terminator = function
            .operations
            .get(block.operation_offset..end)
            .and_then(|operations| operations.last())
            .ok_or_else(invalid)?;
        let successors = match terminator {
            AbstractOperation::Jump { target, .. } => vec![*target],
            AbstractOperation::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.target, when_false.target],
            AbstractOperation::StructuralCase { cases, .. } => {
                cases.iter().map(|case| case.target).collect()
            }
            AbstractOperation::Return { .. }
            | AbstractOperation::ReturnUnit { .. }
            | AbstractOperation::ReturnStructural { .. }
            | AbstractOperation::Crash { .. } => Vec::new(),
            _ => return Err(invalid()),
        };
        outgoing.insert(block.block, successors);
    }
    for successor in outgoing.values().flatten() {
        let count = incoming.get_mut(successor).ok_or_else(invalid)?;
        *count = count.checked_add(1).ok_or_else(invalid)?;
    }
    let mut pending = incoming
        .iter()
        .filter_map(|(block, count)| (*count == 0).then_some(*block))
        .collect::<Vec<_>>();
    let mut visited = 0;
    while let Some(block) = pending.pop() {
        visited += 1;
        for successor in outgoing.get(&block).ok_or_else(invalid)? {
            let count = incoming.get_mut(successor).ok_or_else(invalid)?;
            *count = count.checked_sub(1).ok_or_else(invalid)?;
            if *count == 0 {
                pending.push(*successor);
            }
        }
    }
    Ok(visited != function.block_entries.len())
}
