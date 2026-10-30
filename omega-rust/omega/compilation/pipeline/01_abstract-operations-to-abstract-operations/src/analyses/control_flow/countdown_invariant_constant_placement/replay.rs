//! Optimizer module role: validation leaf. Independently reconstructed countdown placements.

use super::{
    AnalysisRoot, CountdownInvariantConstantConsumer, CountdownInvariantConstantDestination,
    CountdownInvariantConstantPlacement, CountdownInvariantConstantPlacementAnalysisError,
    CountdownInvariantConstantPlacementAnalysisSnapshot, CountdownInvariantConstantRole,
    CountdownInvariantIntegerConstant, NodeLocation, O,
    OptimizerUnsignedCountdownRankingCertificate, PsiOptimizationFunction, PsiOptimizationUnit,
    ScalarType, UnsignedCountdownInvariantConstantPlacements, UnsignedCountdownLoopSummary,
    ValidatedCountdownInvariantConstantAnalysis, ValidatedCountedLoopAnalysis,
    ValidatedOptimizerCycleComponents, by_key, consumer_matches, consumer_operation,
    expected_constant, invariant_constant_row, sole_provenance_node, validate_analysis_root,
    validate_moved_constant_locations,
};

pub(super) fn reconstruct(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
    counted: &ValidatedCountedLoopAnalysis,
    invariants: &ValidatedCountdownInvariantConstantAnalysis,
) -> Result<
    CountdownInvariantConstantPlacementAnalysisSnapshot,
    CountdownInvariantConstantPlacementAnalysisError,
> {
    validate_roots(unit, custody, counted, invariants)?;
    let functions = by_key(&unit.functions, |function| function.machine);
    let components = by_key(custody.components(), |component| component.id.clone());
    let counted_loops = by_key(counted.loops(), |summary| {
        summary.certificate.component.clone()
    });
    let invariant_loops = by_key(invariants.loops(), |summary| {
        summary.counted_loop.certificate.component.clone()
    });
    // The countdown chain is keyed by the certified subset of the validated
    // SCC roster; uncertified components carry no counted or invariant row.
    if functions.len() != unit.functions.len()
        || components.len() != custody.components().len()
        || counted_loops.len() != counted.loops().len()
        || invariant_loops.len() != invariants.loops().len()
        || counted_loops.keys().ne(invariant_loops.keys())
        || counted_loops
            .keys()
            .any(|component| !components.contains_key(component))
    {
        return Err(CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch);
    }

    let mut loops = Vec::with_capacity(custody.ranking_certificates().certificates().len());
    for certificate in custody.ranking_certificates().certificates() {
        let component = components
            .get(&certificate.component)
            .copied()
            .ok_or(CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch)?;
        let counted = counted_loops
            .get(&certificate.component)
            .copied()
            .filter(|summary| summary.certificate == *certificate)
            .ok_or(CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch)?;
        let invariants = invariant_loops
            .get(&certificate.component)
            .copied()
            .filter(|summary| summary.counted_loop == *counted)
            .ok_or(CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch)?;
        let function = functions
            .get(&certificate.component.machine)
            .copied()
            .ok_or(CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch)?;
        let destination = reconstruct_destination(function, component, counted)?;
        let constants = [
            reconstruct_constant(
                function,
                certificate,
                CountdownInvariantConstantRole::PositiveGuardZero,
            )?,
            reconstruct_constant(
                function,
                certificate,
                CountdownInvariantConstantRole::BackedgeDecrementOne,
            )?,
        ];
        validate_constant_locations(function, component, certificate, &constants)?;
        let placements = [
            CountdownInvariantConstantRole::PositiveGuardZero,
            CountdownInvariantConstantRole::BackedgeDecrementOne,
        ]
        .into_iter()
        .map(|role| {
            let constant = constants
                .iter()
                .find(|constant| constant.role == role)
                .cloned()
                .ok_or(CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch)?;
            let retained = invariants
                .constants
                .iter()
                .filter(|candidate| candidate.role == role)
                .collect::<Vec<_>>();
            let [retained] = retained.as_slice() else {
                return Err(
                    CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch,
                );
            };
            if **retained != constant {
                return Err(
                    CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch,
                );
            }
            Ok(CountdownInvariantConstantPlacement {
                consumer: reconstruct_consumer(function, component, certificate, &constant)?,
                constant,
                destination: destination.clone(),
            })
        })
        .collect::<Result<Vec<_>, CountdownInvariantConstantPlacementAnalysisError>>()?;
        if placements.len() != invariants.constants.len() {
            return Err(CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch);
        }
        loops.push(UnsignedCountdownInvariantConstantPlacements {
            component: component.id.clone(),
            counted_loop: counted.clone(),
            placements,
        });
    }
    Ok(CountdownInvariantConstantPlacementAnalysisSnapshot {
        root: AnalysisRoot::of(unit),
        loops,
    })
}

fn reconstruct_destination(
    function: &PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
    counted: &UnsignedCountdownLoopSummary,
) -> Result<CountdownInvariantConstantDestination, CountdownInvariantConstantPlacementAnalysisError>
{
    let [entry_edge] = component.entries.as_slice() else {
        return Err(unsupported(
            function.machine,
            counted.certificate.guard.zero_operation,
        ));
    };
    if *entry_edge != counted.preheader_edge
        || entry_edge.target != counted.certificate.header
        || component.members.contains(&entry_edge.source)
    {
        return Err(unsupported(
            function.machine,
            counted.certificate.guard.zero_operation,
        ));
    }
    let preheader = function
        .block(entry_edge.source)
        .ok_or_else(|| unsupported(function.machine, counted.certificate.guard.zero_operation))?;
    let Some((node_index, terminator)) = preheader.nodes.iter().enumerate().next_back() else {
        return Err(unsupported(
            function.machine,
            counted.certificate.guard.zero_operation,
        ));
    };
    let O::Jump {
        psi_edge,
        target,
        bindings,
        ..
    } = &terminator.operation
    else {
        return Err(unsupported(
            function.machine,
            counted.certificate.guard.zero_operation,
        ));
    };
    let rank_bindings = bindings
        .iter()
        .filter(|binding| binding.parameter == counted.certificate.rank_parameter)
        .collect::<Vec<_>>();
    let [rank_binding] = rank_bindings.as_slice() else {
        return Err(unsupported(
            function.machine,
            counted.certificate.guard.zero_operation,
        ));
    };
    if (*psi_edge, *target) != (entry_edge.edge, counted.certificate.header)
        || rank_binding.argument != counted.trip_count.initial_value
        || rank_binding.scalar_type != ScalarType::Integer(counted.certificate.rank_type)
    {
        return Err(unsupported(
            function.machine,
            counted.certificate.guard.zero_operation,
        ));
    }
    Ok(CountdownInvariantConstantDestination {
        before: NodeLocation {
            machine: function.machine,
            block: preheader.id,
            node: u32::try_from(node_index).map_err(|_| {
                unsupported(function.machine, counted.certificate.guard.zero_operation)
            })?,
        },
        entry_edge: *entry_edge,
    })
}

fn reconstruct_constant(
    function: &PsiOptimizationFunction,
    certificate: &OptimizerUnsignedCountdownRankingCertificate,
    role: CountdownInvariantConstantRole,
) -> Result<CountdownInvariantIntegerConstant, CountdownInvariantConstantPlacementAnalysisError> {
    let expected = expected_constant(certificate, role);
    let (block, node_index, node) =
        sole_provenance_node(function.blocks.iter(), expected.operation)
            .ok_or_else(|| unsupported(function.machine, expected.operation))?;
    invariant_constant_row(function, block, node_index, node, role, expected)
        .ok_or_else(|| unsupported(function.machine, expected.operation))
}

fn validate_constant_locations(
    function: &PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
    certificate: &OptimizerUnsignedCountdownRankingCertificate,
    constants: &[CountdownInvariantIntegerConstant; 2],
) -> Result<(), CountdownInvariantConstantPlacementAnalysisError> {
    validate_moved_constant_locations(function, component, certificate, constants, unsupported)
}

fn reconstruct_consumer(
    function: &PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
    certificate: &OptimizerUnsignedCountdownRankingCertificate,
    constant: &CountdownInvariantIntegerConstant,
) -> Result<CountdownInvariantConstantConsumer, CountdownInvariantConstantPlacementAnalysisError> {
    let operation = consumer_operation(certificate, constant.role);
    let (block, node_index, node) = sole_provenance_node(
        function
            .blocks
            .iter()
            .filter(|block| component.members.contains(&block.id)),
        operation,
    )
    .ok_or_else(|| unsupported(function.machine, operation))?;
    let value_uses = function
        .node_uses()
        .filter(|value_use| value_use.value == constant.result)
        .copied()
        .collect::<Vec<_>>();
    let [value_use] = value_uses.as_slice() else {
        return Err(unsupported(function.machine, operation));
    };
    if (value_use.block, value_use.node) != (block, node_index) {
        return Err(unsupported(function.machine, operation));
    }
    if !consumer_matches(node, certificate, constant.role, operation, constant.result) {
        return Err(unsupported(function.machine, operation));
    }
    Ok(CountdownInvariantConstantConsumer {
        location: NodeLocation {
            machine: function.machine,
            block,
            node: node_index,
        },
        psi_operation: operation,
        value_use: *value_use,
    })
}

fn validate_roots(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
    counted: &ValidatedCountedLoopAnalysis,
    invariants: &ValidatedCountdownInvariantConstantAnalysis,
) -> Result<(), CountdownInvariantConstantPlacementAnalysisError> {
    validate_analysis_root(
        unit,
        custody,
        &[counted.snapshot().root, invariants.snapshot().root],
    )
    .map_err(CountdownInvariantConstantPlacementAnalysisError::Root)?;
    let component_keys = custody
        .components()
        .iter()
        .map(|component| &component.id)
        .collect::<Vec<_>>();
    let certificate_keys = custody
        .ranking_certificates()
        .certificates()
        .iter()
        .map(|certificate| &certificate.component)
        .collect::<Vec<_>>();
    let counted_keys = counted
        .loops()
        .iter()
        .map(|summary| &summary.certificate.component)
        .collect::<Vec<_>>();
    let invariant_keys = invariants
        .loops()
        .iter()
        .map(|summary| &summary.counted_loop.certificate.component)
        .collect::<Vec<_>>();
    // Certificates name the certified subset of the validated SCC roster; the
    // counted and invariant rosters must cover exactly that subset.
    if certificate_keys != counted_keys
        || counted_keys != invariant_keys
        || certificate_keys
            .iter()
            .any(|key| !component_keys.contains(key))
    {
        return Err(CountdownInvariantConstantPlacementAnalysisError::ComponentRosterMismatch);
    }
    Ok(())
}

use super::compute::unsupported;
