use omega_lowering::{BlockTerminator, MachineBody};

/// Walk only the published successors. Every operation must belong to exactly
/// one reachable block, even when a nested arm was emitted out of block-ID order.
pub fn check_operation_ownership(body: &MachineBody) -> Result<(), &'static str> {
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
