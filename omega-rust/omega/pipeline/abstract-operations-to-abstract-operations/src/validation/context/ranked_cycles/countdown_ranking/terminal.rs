//! Optimizer module role: reconstruction leaf. Terminal natural-cycle countdown projection.

use super::super::super::super::{BTreeMap, BTreeSet};
use semantic_vocabulary::IntegerValue;
use semantic_vocabulary::{BlockId, ValueId};

use super::super::{
    CycleComponentEdge, OptimizerCycleComponent, OptimizerUnsignedCountdownRankingCertificate,
    OptimizerUnsignedMinusOneDescent, OptimizerUnsignedPositiveGuard,
};
use super::OptimizationUnitValidationError;
/// Project the exact unsigned-countdown idiom out of one verifier-admitted
/// `Natural` component. A component whose verified ranking is not that shape
/// yields no certificate: it retains its verified source and frozen body
/// without acquiring countdown analysis evidence.
pub(super) fn project(
    module: &::terminal_psi::TerminalModule,
    component: &OptimizerCycleComponent,
) -> Result<Option<OptimizerUnsignedCountdownRankingCertificate>, OptimizationUnitValidationError> {
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == component.id.machine)
        .ok_or_else(|| mismatch(component))?;
    Ok(project_one(machine, component))
}

fn project_one(
    machine: &::terminal_psi::TerminalMachine,
    component: &OptimizerCycleComponent,
) -> Option<OptimizerUnsignedCountdownRankingCertificate> {
    let ::terminal_psi::TerminalRankedScc::Natural(naturals) = machine.ranked_scc.as_ref()?;
    let internal_edges = component
        .id
        .internal_edges
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let natural = naturals.iter().find(|natural| {
        natural
            .ranks
            .iter()
            .map(|rank| rank.block)
            .collect::<Vec<_>>()
            == component.members
            && natural
                .edges
                .iter()
                .map(|edge| CycleComponentEdge {
                    edge: edge.edge,
                    source: edge.source,
                    target: edge.target,
                })
                .collect::<BTreeSet<_>>()
                == internal_edges
    })?;
    let ranks = natural
        .ranks
        .iter()
        .map(|rank| (rank.block, rank.value))
        .collect::<BTreeMap<_, _>>();

    // The countdown idiom crosses exactly one strict edge: the counted
    // backedge returning to the header.
    let strict = natural
        .edges
        .iter()
        .filter(|edge| edge.comparison == ::terminal_psi::TerminalNaturalRankComparison::Strict)
        .collect::<Vec<_>>();
    let [backedge] = strict.as_slice() else {
        return None;
    };
    let header_id = backedge.target;
    let decrement_id = backedge.source;
    let rank_parameter = *ranks.get(&header_id)?;
    let source_parameter = *ranks.get(&decrement_id)?;

    // The header's conditional arm inside the component is the positive guard:
    // `0 < rank_parameter` admits the decrement block.
    let header = block(machine, header_id)?;
    let ::terminal_psi::Terminator::Conditional {
        condition,
        when_true,
        when_false,
    } = &header.terminator
    else {
        return None;
    };
    let guard = [when_true, when_false].into_iter().find(|successor| {
        internal_edges.iter().any(|internal| {
            internal.edge == successor.edge
                && internal.source == header_id
                && internal.target == successor.target
        })
    })?;
    let comparison = scalar_operation(header, *condition)?;
    let ::terminal_psi::OperationKind::IntegerLessThan { left: zero, right } = comparison.kind
    else {
        return None;
    };
    if right != rank_parameter {
        return None;
    }
    let zero_operation = scalar_operation(header, zero)?;
    if zero_operation.kind
        != (::terminal_psi::OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(0),
        })
    {
        return None;
    }

    // The strict successor binds the header's rank parameter through the
    // decrement block's exact `rank - 1` subtraction.
    let argument_index = header
        .parameters
        .iter()
        .position(|parameter| parameter.id == rank_parameter)?;
    let decrement = block(machine, decrement_id)?;
    let ::terminal_psi::Terminator::Jump {
        edge: jump_edge,
        target,
        arguments,
        ..
    } = &decrement.terminator
    else {
        return None;
    };
    if *jump_edge != backedge.edge
        || *target != header_id
        || arguments.get(argument_index).copied() != Some(backedge.successor_rank)
    {
        return None;
    }
    let subtract = scalar_operation(decrement, backedge.successor_rank)?;
    let ::terminal_psi::OperationKind::ExactIntegerSubtract {
        left,
        right: one,
        obligation,
    } = subtract.kind
    else {
        return None;
    };
    if left != source_parameter {
        return None;
    }
    let one_operation = scalar_operation(decrement, one)?;
    if one_operation.kind
        != (::terminal_psi::OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(1),
        })
    {
        return None;
    }
    Some(OptimizerUnsignedCountdownRankingCertificate {
        component: component.id.clone(),
        header: header_id,
        rank_parameter,
        rank_type: natural.rank_type,
        lower_bound: natural.rank_type.minimum_value(),
        upper_bound: natural.rank_type.maximum_value(),
        guard: OptimizerUnsignedPositiveGuard {
            block: header_id,
            edge: guard.edge,
            condition: *condition,
            parameter: rank_parameter,
            zero,
            zero_operation: zero_operation.id,
            comparison_operation: comparison.id,
        },
        descent: OptimizerUnsignedMinusOneDescent {
            backedge: CycleComponentEdge {
                edge: backedge.edge,
                source: decrement_id,
                target: header_id,
            },
            argument_index: u32::try_from(argument_index).ok()?,
            argument: backedge.successor_rank,
            source_parameter,
            target_parameter: rank_parameter,
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
