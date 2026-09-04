//! Optimizer module role: proposal leaf. Direct certificate-owned constant construction.

use super::*;

pub(super) fn propose(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
    counted: &ValidatedCountedLoopAnalysis,
) -> Result<CountdownInvariantConstantAnalysisSnapshot, CountdownInvariantConstantAnalysisError> {
    validate_roots(unit, custody, counted)?;
    let mut loops = Vec::with_capacity(counted.loops().len());
    for summary in counted.loops() {
        let certificate = custody
            .ranking_certificates()
            .certificates()
            .iter()
            .find(|certificate| certificate.component == summary.certificate.component)
            .filter(|certificate| *certificate == &summary.certificate)
            .ok_or(CountdownInvariantConstantAnalysisError::ComponentRosterMismatch)?;
        let component = custody
            .components()
            .iter()
            .find(|component| component.id == certificate.component)
            .ok_or(CountdownInvariantConstantAnalysisError::ComponentRosterMismatch)?;
        let function = unit
            .functions
            .iter()
            .find(|function| function.machine == certificate.component.machine)
            .ok_or(CountdownInvariantConstantAnalysisError::ComponentRosterMismatch)?;
        if summary.region.blocks != component.members {
            return Err(CountdownInvariantConstantAnalysisError::ComponentRosterMismatch);
        }
        let constants = [
            locate(
                function,
                certificate,
                CountdownInvariantConstantRole::PositiveGuardZero,
            )?,
            locate(
                function,
                certificate,
                CountdownInvariantConstantRole::BackedgeDecrementOne,
            )?,
        ];
        validate_locations(function, component, certificate, &constants)?;
        loops.push(UnsignedCountdownInvariantConstants {
            counted_loop: summary.clone(),
            prospective_preheader: summary.preheader_edge.source,
            constants: constants.into(),
        });
    }
    Ok(CountdownInvariantConstantAnalysisSnapshot {
        revision: unit.identity,
        terminal_psi: unit.psi,
        loops,
    })
}

fn locate(
    function: &PsiOptimizationFunction,
    certificate: &OptimizerUnsignedCountdownRankingCertificate,
    role: CountdownInvariantConstantRole,
) -> Result<CountdownInvariantIntegerConstant, CountdownInvariantConstantAnalysisError> {
    let (operation, result, value) = expected(certificate, role);
    let mut matches = function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .nodes
                .iter()
                .enumerate()
                .map(move |(node, value)| (block.id, node, value))
        })
        .filter(|(_, _, node)| match node.operation {
            O::IntegerConstant { psi_operation, .. } => psi_operation == operation,
            _ => false,
        });
    let Some((block, node_index, node)) = matches.next() else {
        return Err(unsupported(function.machine, operation));
    };
    if matches.next().is_some() {
        return Err(unsupported(function.machine, operation));
    }
    row(
        function.machine,
        block,
        node_index,
        node,
        role,
        operation,
        result,
        certificate.rank_type,
        value,
    )
}

fn validate_locations(
    function: &PsiOptimizationFunction,
    component: &omega_optimization_validation::OptimizerCycleComponent,
    certificate: &OptimizerUnsignedCountdownRankingCertificate,
    constants: &[CountdownInvariantIntegerConstant; 2],
) -> Result<(), CountdownInvariantConstantAnalysisError> {
    let original_blocks = [certificate.header, certificate.descent.backedge.source];
    if constants
        .iter()
        .zip(original_blocks)
        .all(|(constant, original)| constant.location.block == original)
    {
        return Ok(());
    }
    let [entry] = component.entries.as_slice() else {
        return Err(unsupported(
            function.machine,
            certificate.guard.zero_operation,
        ));
    };
    if component.members.contains(&entry.source) || entry.target != certificate.header {
        return Err(unsupported(
            function.machine,
            certificate.guard.zero_operation,
        ));
    }
    let preheader = function
        .blocks
        .iter()
        .find(|block| block.id == entry.source)
        .ok_or_else(|| unsupported(function.machine, certificate.guard.zero_operation))?;
    let jump_index = preheader
        .nodes
        .len()
        .checked_sub(1)
        .ok_or_else(|| unsupported(function.machine, certificate.guard.zero_operation))?;
    let O::Jump {
        psi_edge, target, ..
    } = preheader.nodes[jump_index].operation
    else {
        return Err(unsupported(
            function.machine,
            certificate.guard.zero_operation,
        ));
    };
    if psi_edge != entry.edge || target != certificate.header {
        return Err(unsupported(
            function.machine,
            certificate.guard.zero_operation,
        ));
    }
    let moved = constants
        .iter()
        .zip(original_blocks)
        .filter_map(|(constant, original)| {
            (constant.location.block != original).then_some(constant)
        })
        .collect::<Vec<_>>();
    let suffix_start = jump_index
        .checked_sub(moved.len())
        .ok_or_else(|| unsupported(function.machine, certificate.guard.zero_operation))?;
    if moved.iter().enumerate().any(|(offset, constant)| {
        constant.location.machine != function.machine
            || constant.location.block != preheader.id
            || usize::try_from(constant.location.node).ok() != Some(suffix_start + offset)
    }) {
        return Err(unsupported(
            function.machine,
            certificate.guard.zero_operation,
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn row(
    machine: MachineId,
    block: BlockId,
    node_index: usize,
    node: &omega_optimization_unit::OptimizationNode,
    role: CountdownInvariantConstantRole,
    expected_operation: OperationId,
    expected_result: ValueId,
    expected_type: IntegerType,
    expected_value: IntegerValue,
) -> Result<CountdownInvariantIntegerConstant, CountdownInvariantConstantAnalysisError> {
    let location = NodeLocation {
        machine,
        block,
        node: u32::try_from(node_index).map_err(|_| unsupported(machine, expected_operation))?,
    };
    let O::IntegerConstant {
        psi_operation,
        result,
        scalar_type: ScalarType::Integer(scalar_type),
        value,
    } = node.operation
    else {
        return Err(unsupported(machine, expected_operation));
    };
    let [definition] = node.definitions.as_slice() else {
        return Err(unsupported(machine, psi_operation));
    };
    if psi_operation != expected_operation
        || result != expected_result
        || scalar_type != expected_type
        || value != expected_value
        || definition.value != result
        || definition.scalar_type != ScalarType::Integer(scalar_type)
        || definition.site
            != (ValueDefinitionSite::Node {
                block,
                node: location.node,
            })
        || !node.uses.is_empty()
        || !node.successors.is_empty()
        || !node.ownership.is_empty()
    {
        return Err(unsupported(machine, psi_operation));
    }
    Ok(CountdownInvariantIntegerConstant {
        role,
        location,
        psi_operation,
        result,
        scalar_type,
        value,
        definition: *definition,
        provenance: node.provenance.clone(),
        fuel: node.fuel.clone(),
        effect: node.effect,
    })
}

fn expected(
    certificate: &OptimizerUnsignedCountdownRankingCertificate,
    role: CountdownInvariantConstantRole,
) -> (OperationId, ValueId, IntegerValue) {
    match role {
        CountdownInvariantConstantRole::PositiveGuardZero => (
            certificate.guard.zero_operation,
            certificate.guard.zero,
            IntegerValue::Unsigned(0),
        ),
        CountdownInvariantConstantRole::BackedgeDecrementOne => (
            certificate.descent.one_operation,
            certificate.descent.one,
            IntegerValue::Unsigned(1),
        ),
    }
}

fn validate_roots(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
    counted: &ValidatedCountedLoopAnalysis,
) -> Result<(), CountdownInvariantConstantAnalysisError> {
    let recomputed = recompute_psi_optimization_unit_identity(unit);
    if recomputed != unit.identity {
        return Err(CountdownInvariantConstantAnalysisError::StaleUnitIdentity {
            stored: unit.identity,
            recomputed,
        });
    }
    if unit.psi != custody.terminal_psi()
        || unit.psi != custody.ranking_certificates().snapshot().terminal_psi
        || unit.psi != counted.snapshot().terminal_psi
    {
        return Err(CountdownInvariantConstantAnalysisError::TerminalIdentityMismatch);
    }
    if counted.snapshot().revision != unit.identity {
        return Err(CountdownInvariantConstantAnalysisError::CountedLoopRevisionMismatch);
    }
    if custody.components().len() != custody.ranking_certificates().certificates().len()
        || custody.components().len() != counted.loops().len()
    {
        return Err(CountdownInvariantConstantAnalysisError::ComponentRosterMismatch);
    }
    Ok(())
}

fn unsupported(
    machine: MachineId,
    operation: OperationId,
) -> CountdownInvariantConstantAnalysisError {
    CountdownInvariantConstantAnalysisError::UnsupportedInvariantConstant { machine, operation }
}
