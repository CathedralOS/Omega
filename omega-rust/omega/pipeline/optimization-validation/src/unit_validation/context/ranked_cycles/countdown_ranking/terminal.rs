//! Optimizer module role: reconstruction leaf. Terminal countdown evidence projection.

use super::*;

pub(super) fn derive(
    module: &::terminal_psi::TerminalModule,
    snapshot: &OptimizerCycleComponentSnapshot,
) -> Result<Vec<OptimizerUnsignedCountdownRankingCertificate>, OptimizationUnitValidationError> {
    let mut certificates = Vec::new();
    for component in &snapshot.components {
        let machine = module
            .machines
            .iter()
            .find(|machine| machine.id == component.id.machine)
            .ok_or_else(|| mismatch(component))?;
        certificates.push(derive_one(machine, component)?);
    }
    certificates.sort_by(|left, right| left.component.cmp(&right.component));
    Ok(certificates)
}

fn derive_one(
    machine: &::terminal_psi::TerminalMachine,
    component: &OptimizerCycleComponent,
) -> Result<OptimizerUnsignedCountdownRankingCertificate, OptimizationUnitValidationError> {
    let ranked = machine
        .ranked_scc
        .as_ref()
        .ok_or_else(|| mismatch(component))?;
    let [covered] = ranked.covered_cyclic_edges.as_slice() else {
        return Err(mismatch(component));
    };
    let ::terminal_psi::TerminalRankedGuard::UnsignedParameterPositive {
        block: guard_block,
        edge: guard_edge,
        condition,
        parameter,
    } = covered.guard;
    let ::terminal_psi::TerminalRankedSuccessorArgument::UnsignedParameterMinusOne {
        argument_index,
        argument,
        source_parameter,
        target_parameter,
    } = covered.successor_argument;
    let header = block(machine, ranked.header).ok_or_else(|| mismatch(component))?;
    let comparison = scalar_operation(header, condition).ok_or_else(|| mismatch(component))?;
    let ::terminal_psi::OperationKind::IntegerLessThan { left: zero, right } = comparison.kind
    else {
        return Err(mismatch(component));
    };
    if right != ranked.rank_parameter {
        return Err(mismatch(component));
    }
    let zero_operation = scalar_operation(header, zero).ok_or_else(|| mismatch(component))?;
    if zero_operation.kind
        != (::terminal_psi::OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(0),
        })
    {
        return Err(mismatch(component));
    }
    let decrement = block(machine, covered.source).ok_or_else(|| mismatch(component))?;
    let subtract = scalar_operation(decrement, argument).ok_or_else(|| mismatch(component))?;
    let ::terminal_psi::OperationKind::ExactIntegerSubtract {
        left,
        right: one,
        obligation,
    } = subtract.kind
    else {
        return Err(mismatch(component));
    };
    if left != source_parameter {
        return Err(mismatch(component));
    }
    let one_operation = scalar_operation(decrement, one).ok_or_else(|| mismatch(component))?;
    if one_operation.kind
        != (::terminal_psi::OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(1),
        })
    {
        return Err(mismatch(component));
    }
    let backedge = CycleComponentEdge {
        edge: covered.edge,
        source: covered.source,
        target: covered.target,
    };
    if !component.id.internal_edges.contains(&backedge) {
        return Err(mismatch(component));
    }
    Ok(OptimizerUnsignedCountdownRankingCertificate {
        component: component.id.clone(),
        header: ranked.header,
        rank_parameter: ranked.rank_parameter,
        rank_type: ranked.rank_type,
        lower_bound: ranked.lower_bound,
        upper_bound: ranked.upper_bound,
        guard: OptimizerUnsignedPositiveGuard {
            block: guard_block,
            edge: guard_edge,
            condition,
            parameter,
            zero,
            zero_operation: zero_operation.id,
            comparison_operation: comparison.id,
        },
        descent: OptimizerUnsignedMinusOneDescent {
            backedge,
            argument_index,
            argument,
            source_parameter,
            target_parameter,
            one,
            one_operation: one_operation.id,
            subtract_operation: subtract.id,
            subtract_obligation: obligation,
        },
    })
}

fn block(machine: &::terminal_psi::TerminalMachine, id: BlockId) -> Option<&::terminal_psi::Block> {
    machine.blocks.iter().find(|block| block.id == id)
}

fn scalar_operation(
    block: &::terminal_psi::Block,
    value: ValueId,
) -> Option<&::terminal_psi::Operation> {
    block
        .operations
        .iter()
        .find(|operation| operation.result.scalar().map(|result| result.id) == Some(value))
}

fn mismatch(component: &OptimizerCycleComponent) -> OptimizationUnitValidationError {
    OptimizationUnitValidationError::RankedCycleRankingEvidenceMismatch {
        machine: component.id.machine,
    }
}
