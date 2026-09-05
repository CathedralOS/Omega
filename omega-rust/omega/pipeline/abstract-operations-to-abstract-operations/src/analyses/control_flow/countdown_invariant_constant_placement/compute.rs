//! Optimizer module role: proposal leaf. Certificate-keyed placement construction.

use super::*;

pub(super) fn propose(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
    counted: &ValidatedCountedLoopAnalysis,
    invariants: &ValidatedCountdownInvariantConstantAnalysis,
) -> Result<
    CountdownInvariantConstantPlacementAnalysisSnapshot,
    CountdownInvariantConstantPlacementAnalysisError,
> {
    validate_roots(unit, custody, counted, invariants)?;
    let mut loops = Vec::with_capacity(invariants.loops().len());
    for invariant in invariants.loops() {
        let certificate = &invariant.counted_loop.certificate;
        let component = custody
            .components()
            .iter()
            .find(|component| component.id == certificate.component)
            .ok_or(CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch)?;
        let function = unit
            .functions
            .iter()
            .find(|function| function.machine == certificate.component.machine)
            .ok_or(CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch)?;
        let destination = destination(function, component, &invariant.counted_loop)?;
        let placements = invariant
            .constants
            .iter()
            .map(|constant| {
                Ok(CountdownInvariantConstantPlacement {
                    constant: constant.clone(),
                    destination: destination.clone(),
                    consumer: consumer(function, component, certificate, constant)?,
                })
            })
            .collect::<Result<Vec<_>, CountdownInvariantConstantPlacementAnalysisError>>()?;
        loops.push(UnsignedCountdownInvariantConstantPlacements {
            component: component.id.clone(),
            counted_loop: invariant.counted_loop.clone(),
            placements,
        });
    }
    Ok(CountdownInvariantConstantPlacementAnalysisSnapshot {
        revision: unit.identity,
        terminal_psi: unit.psi,
        loops,
    })
}

fn destination(
    function: &PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
    counted: &UnsignedCountdownLoopSummary,
) -> Result<CountdownInvariantConstantDestination, CountdownInvariantConstantPlacementAnalysisError>
{
    if component.members.contains(&counted.preheader_edge.source) {
        return Err(unsupported(
            function.machine,
            counted.certificate.guard.zero_operation,
        ));
    }
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == counted.preheader_edge.source)
        .ok_or_else(|| unsupported(function.machine, counted.certificate.guard.zero_operation))?;
    let node = block
        .nodes
        .last()
        .ok_or_else(|| unsupported(function.machine, counted.certificate.guard.zero_operation))?;
    let O::Jump {
        psi_edge, target, ..
    } = node.operation
    else {
        return Err(unsupported(
            function.machine,
            counted.certificate.guard.zero_operation,
        ));
    };
    if psi_edge != counted.preheader_edge.edge
        || target != counted.certificate.header
        || counted.preheader_edge.target != counted.certificate.header
        || component.entries.as_slice() != [counted.preheader_edge]
    {
        return Err(unsupported(
            function.machine,
            counted.certificate.guard.zero_operation,
        ));
    }
    Ok(CountdownInvariantConstantDestination {
        before: NodeLocation {
            machine: function.machine,
            block: block.id,
            node: u32::try_from(block.nodes.len() - 1).map_err(|_| {
                unsupported(function.machine, counted.certificate.guard.zero_operation)
            })?,
        },
        entry_edge: counted.preheader_edge,
    })
}

fn consumer(
    function: &PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
    certificate: &OptimizerUnsignedCountdownRankingCertificate,
    constant: &CountdownInvariantIntegerConstant,
) -> Result<CountdownInvariantConstantConsumer, CountdownInvariantConstantPlacementAnalysisError> {
    let operation = expected_consumer(certificate, constant.role);
    let uses = function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().flat_map(|node| &node.uses))
        .filter(|value_use| value_use.value == constant.result)
        .copied()
        .collect::<Vec<_>>();
    let [value_use] = uses.as_slice() else {
        return Err(unsupported(function.machine, constant.psi_operation));
    };
    if !component.members.contains(&value_use.block) {
        return Err(unsupported(function.machine, constant.psi_operation));
    }
    let node = node_at(function, value_use.block, value_use.node)
        .ok_or_else(|| unsupported(function.machine, constant.psi_operation))?;
    check_consumer(node, certificate, constant.role, operation, constant.result)?;
    Ok(CountdownInvariantConstantConsumer {
        location: NodeLocation {
            machine: function.machine,
            block: value_use.block,
            node: value_use.node,
        },
        psi_operation: operation,
        value_use: *value_use,
    })
}

fn check_consumer(
    node: &OptimizationNode,
    certificate: &OptimizerUnsignedCountdownRankingCertificate,
    role: CountdownInvariantConstantRole,
    operation: OperationId,
    constant: ValueId,
) -> Result<(), CountdownInvariantConstantPlacementAnalysisError> {
    let matches = match (role, &node.operation) {
        (
            CountdownInvariantConstantRole::PositiveGuardZero,
            O::IntegerLessThan {
                psi_operation,
                result,
                left,
                right,
            },
        ) => {
            (*psi_operation, *result, *left, *right)
                == (
                    operation,
                    certificate.guard.condition,
                    constant,
                    certificate.rank_parameter,
                )
        }
        (
            CountdownInvariantConstantRole::BackedgeDecrementOne,
            O::ExactIntegerSubtract {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
        ) => {
            (
                *psi_operation,
                *obligation,
                *result,
                *scalar_type,
                *left,
                *right,
            ) == (
                operation,
                certificate.descent.subtract_obligation,
                certificate.descent.argument,
                certificate.rank_type,
                certificate.descent.source_parameter,
                constant,
            )
        }
        _ => false,
    };
    if !matches {
        return Err(unsupported(certificate.component.machine, operation));
    }
    Ok(())
}

fn node_at(
    function: &PsiOptimizationFunction,
    block: BlockId,
    node: u32,
) -> Option<&OptimizationNode> {
    function
        .blocks
        .iter()
        .find(|candidate| candidate.id == block)?
        .nodes
        .get(usize::try_from(node).ok()?)
}

fn expected_consumer(
    certificate: &OptimizerUnsignedCountdownRankingCertificate,
    role: CountdownInvariantConstantRole,
) -> OperationId {
    match role {
        CountdownInvariantConstantRole::PositiveGuardZero => certificate.guard.comparison_operation,
        CountdownInvariantConstantRole::BackedgeDecrementOne => {
            certificate.descent.subtract_operation
        }
    }
}

fn validate_roots(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
    counted: &ValidatedCountedLoopAnalysis,
    invariants: &ValidatedCountdownInvariantConstantAnalysis,
) -> Result<(), CountdownInvariantConstantPlacementAnalysisError> {
    let recomputed = recompute_psi_optimization_unit_identity(unit);
    if recomputed != unit.identity {
        return Err(
            CountdownInvariantConstantPlacementAnalysisError::StaleUnitIdentity {
                stored: unit.identity,
                recomputed,
            },
        );
    }
    if unit.psi != custody.terminal_psi()
        || unit.psi != custody.ranking_certificates().snapshot().terminal_psi
        || unit.psi != counted.snapshot().terminal_psi
        || unit.psi != invariants.snapshot().terminal_psi
    {
        return Err(CountdownInvariantConstantPlacementAnalysisError::TerminalIdentityMismatch);
    }
    if counted.snapshot().revision != unit.identity
        || invariants.snapshot().revision != unit.identity
    {
        return Err(CountdownInvariantConstantPlacementAnalysisError::AnalysisRevisionMismatch);
    }
    if custody.components().len() != counted.loops().len()
        || custody.components().len() != invariants.loops().len()
        || counted
            .loops()
            .iter()
            .zip(invariants.loops())
            .any(|(counted, invariant)| counted != &invariant.counted_loop)
    {
        return Err(CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch);
    }
    Ok(())
}

fn unsupported(
    machine: MachineId,
    operation: OperationId,
) -> CountdownInvariantConstantPlacementAnalysisError {
    CountdownInvariantConstantPlacementAnalysisError::UnsupportedPlacement { machine, operation }
}
