//! A checker-side reading of published natural ranks, from Psi alone. It
//! evaluates a member's entry rank and re-derives an edge's successor rank
//! from the claimed call's own arguments. It judges no comparison, no
//! definedness proof and no termination.
use omega_psi::{BlockTerminator, CallArgument, CallSite, LoweredPackage, MachineId, OperationKind, RankExpression, RankOperand, RankedEdge, ValueId};

/// The natural rank at the given entry formals. `None` where the rank is
/// undefined (an exact difference below zero) or names a missing formal.
pub fn entry_rank(
    rank: &RankExpression<usize>,
    formals: &[u64],
) -> Option<i128> {
    let value = |operand: &RankOperand<usize>| match operand {
        RankOperand::Input(formal) => formals.get(*formal).map(|&v| i128::from(v)),
        RankOperand::Constant(constant) => i128::try_from(constant).ok(),
    };
    match rank {
        RankExpression::Operand(operand) => value(operand),
        RankExpression::SaturatingDistance { cursor, limit } => Some((value(limit)? - value(cursor)?).max(0)),
        RankExpression::ExactDifference { minuend, subtrahend } => Some(value(minuend)? - value(subtrahend)?).filter(|rank| *rank >= 0),
    }
}

/// The target's published entry rank with each formal replaced by the scalar
/// actual of the claimed call, found in the claimed block of the caller.
/// `Err` names the first missing piece.
pub fn successor_from_call(
    package: &LoweredPackage,
    caller: MachineId,
    edge: &RankedEdge,
) -> Result<RankExpression<ValueId>, String> {
    let machine = |id: MachineId| {
        package
            .machines
            .iter()
            .find(|m| m.id == id)
            .ok_or_else(|| format!("no machine {id:?}"))
    };
    let (caller, target) = (machine(caller)?, machine(edge.target)?);
    let block = caller
        .body
        .blocks
        .iter()
        .find(|block| block.id == edge.block)
        .ok_or("the claimed block is not in the caller")?;
    let arguments = match edge.call {
        CallSite::Edge(id) => match &block.terminator {
            BlockTerminator::TerminalCall { id: edge_id, arguments, .. } if *edge_id == id => Some(arguments),
            _ => None,
        },
        CallSite::Operation(id) => caller.body.operations[block.operations.clone()]
            .iter()
            .find(|op| op.id == id)
            .and_then(|op| match &op.kind {
                OperationKind::Call { arguments, .. } => Some(arguments),
                _ => None,
            }),
    }
    .ok_or("the claimed call is not in the claimed block")?;
    let rank = &target
        .contract
        .ranking
        .as_ref()
        .ok_or("the target publishes no rank")?
        .rank;
    rank.try_map(|formal| match arguments.get(formal) {
        Some(CallArgument::Scalar(value)) => Some(*value),
        _ => None,
    })
    .ok_or_else(|| "a rank formal has no scalar actual at the call".into())
}
