use omega_psi::{BlockTerminator, MachineBody, OperationResult, PlaceOrigin, ValueOrigin};

/// Walk only the published successors. Every operation must belong to exactly
/// one reachable block, even when a nested arm was emitted out of block-ID order.
pub fn check_operation_ownership(body: &MachineBody) -> Result<(), &'static str> {
    for (index, operation) in body.operations.iter().enumerate() {
        if operation.id.index() != index {
            return Err("operation identity disagrees with its slot");
        }
        let defines_result = match operation.result {
            OperationResult::Unit => true,
            OperationResult::Scalar(value) => body
                .values
                .get(value.index())
                .is_some_and(|v| v.origin == ValueOrigin::Operation(operation.id)),
            OperationResult::Structural(place) => body
                .places
                .get(place.index())
                .is_some_and(|p| p.origin == PlaceOrigin::Operation(operation.id)),
        };
        if !defines_result {
            return Err("result does not name its defining operation");
        }
    }
    let mut pending = vec![0];
    let mut reached = vec![false; body.blocks.len()];
    let mut owned = vec![false; body.operations.len()];
    while let Some(index) = pending.pop() {
        let block = body.blocks.get(index).ok_or("edge has no destination block")?;
        if block.id.get() != index + 1 {
            return Err("block identity disagrees with its destination");
        }
        if std::mem::replace(&mut reached[index], true) {
            continue;
        }
        for taken in owned
            .get_mut(block.operations.clone())
            .ok_or("block operation range is invalid")?
        {
            if std::mem::replace(taken, true) {
                return Err("operation belongs to multiple reachable blocks");
            }
        }
        match &block.terminator {
            BlockTerminator::Conditional { when_true, when_false, .. } => {
                pending.push(when_true.target.get() - 1);
                pending.push(when_false.target.get() - 1);
            }
            BlockTerminator::Jump(edge) => pending.push(edge.target.get() - 1),
            BlockTerminator::Return { .. } | BlockTerminator::TerminalCall { .. } | BlockTerminator::ExternalExit(_) => {}
        }
    }
    if reached.contains(&false) {
        return Err("block has no path from the entry");
    }
    if owned.contains(&false) {
        return Err("operation belongs to no reachable block");
    }
    Ok(())
}
