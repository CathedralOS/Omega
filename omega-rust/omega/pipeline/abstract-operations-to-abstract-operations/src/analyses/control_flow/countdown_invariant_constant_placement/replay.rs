//! Optimizer module role: validation leaf. Independently reconstructed countdown placements.

use std::collections::BTreeMap;

use super::*;

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
    let functions = unit
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect::<BTreeMap<_, _>>();
    let components = custody
        .components()
        .iter()
        .map(|component| (component.id.clone(), component))
        .collect::<BTreeMap<_, _>>();
    let counted_loops = counted
        .loops()
        .iter()
        .map(|summary| (summary.certificate.component.clone(), summary))
        .collect::<BTreeMap<_, _>>();
    let invariant_loops = invariants
        .loops()
        .iter()
        .map(|summary| (summary.counted_loop.certificate.component.clone(), summary))
        .collect::<BTreeMap<_, _>>();
    if functions.len() != unit.functions.len()
        || components.len() != custody.components().len()
        || counted_loops.len() != counted.loops().len()
        || invariant_loops.len() != invariants.loops().len()
        || components.keys().ne(counted_loops.keys())
        || components.keys().ne(invariant_loops.keys())
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
        revision: unit.identity,
        terminal_psi: unit.psi,
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
        .blocks
        .iter()
        .find(|block| block.id == entry_edge.source)
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
    let (operation, result, value) = match role {
        CountdownInvariantConstantRole::PositiveGuardZero => (
            certificate.guard.zero_operation,
            certificate.guard.zero,
            semantic_vocabulary::IntegerValue::Unsigned(0),
        ),
        CountdownInvariantConstantRole::BackedgeDecrementOne => (
            certificate.descent.one_operation,
            certificate.descent.one,
            semantic_vocabulary::IntegerValue::Unsigned(1),
        ),
    };
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
        .filter(|(_, _, node)| {
            node.provenance
                .first()
                .is_some_and(|source| *source == PsiProvenance::Operation(operation))
        });
    let Some((block, node_index, node)) = matches.next() else {
        return Err(unsupported(function.machine, operation));
    };
    if matches.next().is_some() {
        return Err(unsupported(function.machine, operation));
    }
    let node_index =
        u32::try_from(node_index).map_err(|_| unsupported(function.machine, operation))?;
    let O::IntegerConstant {
        psi_operation,
        result: actual_result,
        scalar_type,
        value: actual_value,
    } = node.operation
    else {
        return Err(unsupported(function.machine, operation));
    };
    let [definition] = node.definitions.as_slice() else {
        return Err(unsupported(function.machine, operation));
    };
    if psi_operation != operation
        || actual_result != result
        || scalar_type != ScalarType::Integer(certificate.rank_type)
        || actual_value != value
        || definition.value != result
        || definition.scalar_type != ScalarType::Integer(certificate.rank_type)
        || definition.site
            != (ValueDefinitionSite::Node {
                block,
                node: node_index,
            })
        || !node.uses.is_empty()
        || !node.successors.is_empty()
        || !node.ownership.is_empty()
    {
        return Err(unsupported(function.machine, operation));
    }
    Ok(CountdownInvariantIntegerConstant {
        role,
        location: NodeLocation {
            machine: function.machine,
            block,
            node: node_index,
        },
        psi_operation,
        result,
        scalar_type: certificate.rank_type,
        value,
        definition: *definition,
        provenance: node.provenance.clone(),
        fuel: node.fuel.clone(),
        effect: node.effect,
    })
}

fn validate_constant_locations(
    function: &PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
    certificate: &OptimizerUnsignedCountdownRankingCertificate,
    constants: &[CountdownInvariantIntegerConstant; 2],
) -> Result<(), CountdownInvariantConstantPlacementAnalysisError> {
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

fn reconstruct_consumer(
    function: &PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
    certificate: &OptimizerUnsignedCountdownRankingCertificate,
    constant: &CountdownInvariantIntegerConstant,
) -> Result<CountdownInvariantConstantConsumer, CountdownInvariantConstantPlacementAnalysisError> {
    let operation = match constant.role {
        CountdownInvariantConstantRole::PositiveGuardZero => certificate.guard.comparison_operation,
        CountdownInvariantConstantRole::BackedgeDecrementOne => {
            certificate.descent.subtract_operation
        }
    };
    let mut matches = function
        .blocks
        .iter()
        .filter(|block| component.members.contains(&block.id))
        .flat_map(|block| {
            block
                .nodes
                .iter()
                .enumerate()
                .map(move |(node, value)| (block.id, node, value))
        })
        .filter(|(_, _, node)| {
            node.provenance
                .first()
                .is_some_and(|source| *source == PsiProvenance::Operation(operation))
        });
    let Some((block, node_index, node)) = matches.next() else {
        return Err(unsupported(function.machine, operation));
    };
    if matches.next().is_some() {
        return Err(unsupported(function.machine, operation));
    }
    let node_index =
        u32::try_from(node_index).map_err(|_| unsupported(function.machine, operation))?;
    let value_uses = function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().flat_map(|node| &node.uses))
        .filter(|value_use| value_use.value == constant.result)
        .copied()
        .collect::<Vec<_>>();
    let [value_use] = value_uses.as_slice() else {
        return Err(unsupported(function.machine, operation));
    };
    if (value_use.block, value_use.node) != (block, node_index) {
        return Err(unsupported(function.machine, operation));
    }
    let shape_matches = match (constant.role, &node.operation) {
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
                    constant.result,
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
                constant.result,
            )
        }
        _ => false,
    };
    if !shape_matches {
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
    let recomputed = recompute_psi_optimization_unit_identity(unit);
    if recomputed != unit.identity {
        return Err(
            CountdownInvariantConstantPlacementAnalysisError::StaleUnitIdentity {
                stored: unit.identity,
                recomputed,
            },
        );
    }
    if unit.psi != custody.snapshot().terminal_psi
        || unit.psi != custody.ranking_certificates().snapshot().terminal_psi
        || unit.psi != counted.snapshot().terminal_psi
        || unit.psi != invariants.snapshot().terminal_psi
    {
        return Err(CountdownInvariantConstantPlacementAnalysisError::TerminalIdentityMismatch);
    }
    if unit.identity != counted.snapshot().revision
        || unit.identity != invariants.snapshot().revision
    {
        return Err(CountdownInvariantConstantPlacementAnalysisError::AnalysisRevisionMismatch);
    }
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
    if component_keys != certificate_keys
        || component_keys != counted_keys
        || component_keys != invariant_keys
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
