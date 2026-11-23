//! A checker-side reading of published natural ranks, from Psi alone. It
//! evaluates a member's entry rank and re-derives an edge's successor rank
//! from the claimed call's own arguments. It judges no comparison, no
//! definedness proof and no termination.
use omega_psi::{
    BlockTerminator, BranchBlock, CallArgument, CallSite, CleanupAction, CleanupList, CleanupSite, LoweredMachine, LoweredPackage, MachineId, OperationKind,
    ProjectedPlace, RankExpression, RankOperand, RankedEdge, StructuralStoreDisposition, TypeId, ValueId,
};

/// The natural rank at the given entry formals. `None` where the rank is
/// undefined, names a missing formal, or exceeds this reader's u64 addition
/// and subtraction fragment. It reads the original carrier policy at each step.
pub fn entry_rank(
    rank: &RankExpression<usize>,
    formals: &[u64],
) -> Option<i128> {
    let value = |operand: &RankOperand<usize>| match operand {
        RankOperand::Input(formal) => formals.get(*formal).map(|&v| i128::from(v)),
        RankOperand::Constant(constant) => i128::try_from(constant).ok(),
    };
    match rank {
        RankExpression::Arithmetic(steps) => {
            use omega_psi::{ArithmeticPolicy, IntegerObligation, IntegerPrimitive, RankArithmeticStep, ScalarType};
            let mut stack: Vec<u64> = Vec::new();
            for step in steps {
                let result = match step {
                    RankArithmeticStep::Operand { operand, carrier: ScalarType::Integer {signed: false, width: 64} } => u64::try_from(value(operand)?).ok()?,
                    RankArithmeticStep::Operand {..} => return None,
                    RankArithmeticStep::Apply(op) => {
                        let right = stack.pop()?;
                        let left = stack.pop()?;
                        match (op.primitive, op.policy, op.obligations.as_slice()) {
                            (IntegerPrimitive::Add, ArithmeticPolicy::Exact, [IntegerObligation::ResultFitsCarrier]) => left.checked_add(right)?,
                            (IntegerPrimitive::Add, ArithmeticPolicy::Wrapping, []) => left.wrapping_add(right),
                            (IntegerPrimitive::Add, ArithmeticPolicy::Saturating, []) => left.saturating_add(right),
                            (IntegerPrimitive::Subtract, ArithmeticPolicy::Exact, [IntegerObligation::ResultFitsCarrier]) => left.checked_sub(right)?,
                            (IntegerPrimitive::Subtract, ArithmeticPolicy::Wrapping, []) => left.wrapping_sub(right),
                            (IntegerPrimitive::Subtract, ArithmeticPolicy::Saturating, []) => left.saturating_sub(right),
                            _ => return None,
                        }
                    }
                };
                stack.push(result);
            }
            let [result] = stack.as_slice() else { return None; };
            Some(i128::from(*result))
        }
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
    let block = claimed_block(caller, edge)?;
    let rank = &target
        .contract
        .ranking
        .as_ref()
        .ok_or("the target publishes no rank")?
        .rank;
    let arguments = match edge.call {
        // The cleaned place is the only actual, and it is structural: only a
        // constant target rank substitutes.
        CallSite::Cleanup(at) => {
            let (_, _, invoked) = cleanup_in_block(block, caller, at)?;
            if invoked != edge.target {
                return Err("the claimed cleanup invokes another machine".into());
            }
            return rank
                .try_map(|_| None)
                .ok_or_else(|| "a rank formal has no scalar actual at the cleanup".into());
        }
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
    rank.try_map(|formal| match arguments.get(formal) {
        Some(CallArgument::Scalar(value)) => Some(*value),
        _ => None,
    })
    .ok_or_else(|| "a rank formal has no scalar actual at the call".into())
}

fn claimed_block<'a>(
    caller: &'a LoweredMachine,
    edge: &RankedEdge,
) -> Result<&'a BranchBlock, String> {
    caller
        .body
        .blocks
        .iter()
        .find(|block| block.id == edge.block)
        .ok_or_else(|| "the claimed block is not in the caller".into())
}

/// The cleaned place, its type and the invoked machine of a claimed cleanup,
/// read from the caller's published block by list and ordinal alone.
pub fn claimed_cleanup<'a>(
    caller: &'a LoweredMachine,
    edge: &RankedEdge,
) -> Result<(&'a ProjectedPlace, TypeId, MachineId), String> {
    let CallSite::Cleanup(at) = edge.call else {
        return Err("the claim is not a cleanup invocation".into());
    };
    cleanup_in_block(claimed_block(caller, edge)?, caller, at)
}

fn cleanup_in_block<'a>(
    block: &'a BranchBlock,
    caller: &'a LoweredMachine,
    at: CleanupSite,
) -> Result<(&'a ProjectedPlace, TypeId, MachineId), String> {
    let list = match at.list {
        CleanupList::Edge(id) => match &block.terminator {
            BlockTerminator::Return { id: edge, cleanup, .. } | BlockTerminator::TerminalCall { id: edge, cleanup, .. } if *edge == id => Some(cleanup),
            BlockTerminator::Jump(edge) if edge.id == id => Some(&edge.cleanup),
            BlockTerminator::Conditional { when_true, when_false, .. } => [when_true, when_false]
                .into_iter()
                .find(|edge| edge.id == id)
                .map(|edge| &edge.cleanup),
            _ => None,
        },
        CleanupList::Store(id) => caller.body.operations[block.operations.clone()]
            .iter()
            .find(|op| op.id == id)
            .and_then(|op| match &op.kind {
                OperationKind::StructuralStore {
                    disposition: StructuralStoreDisposition::Dispose(actions),
                    ..
                } => Some(actions),
                _ => None,
            }),
    }
    .ok_or("the claimed cleanup list is not in the claimed block")?;
    match list.get(at.action) {
        Some(CleanupAction::Invoke { place, ty, target }) => Ok((place, *ty, *target)),
        _ => Err("the claimed cleanup action is not an invocation".into()),
    }
}
