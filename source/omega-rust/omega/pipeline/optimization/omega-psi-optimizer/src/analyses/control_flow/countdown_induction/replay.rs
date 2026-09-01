//! Optimizer module role: validation leaf. Independently keyed countdown-summary reconstruction.

use std::collections::BTreeMap;

use super::*;

pub(super) fn reconstruct(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
) -> Result<CountedLoopAnalysisSnapshot, CountedLoopAnalysisError> {
    let recomputed = recompute_psi_optimization_unit_identity(unit);
    if unit.identity != recomputed {
        return Err(CountedLoopAnalysisError::StaleUnitIdentity {
            stored: unit.identity,
            recomputed,
        });
    }
    if unit.psi != custody.snapshot().terminal_psi
        || unit.psi != custody.ranking_certificates().snapshot().terminal_psi
    {
        return Err(CountedLoopAnalysisError::TerminalIdentityMismatch);
    }
    let components = custody
        .components()
        .iter()
        .map(|component| (component.id.clone(), component))
        .collect::<BTreeMap<_, _>>();
    let certificates = custody
        .ranking_certificates()
        .certificates()
        .iter()
        .map(|certificate| (certificate.component.clone(), certificate))
        .collect::<BTreeMap<_, _>>();
    if components.len() != custody.components().len()
        || certificates.len() != custody.ranking_certificates().certificates().len()
        || components.keys().ne(certificates.keys())
    {
        return Err(CountedLoopAnalysisError::CertificateComponentRosterMismatch);
    }
    let functions = unit
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect::<BTreeMap<_, _>>();
    let mut loops = Vec::with_capacity(certificates.len());
    for (id, certificate) in certificates {
        let component = components[&id];
        let function = functions
            .get(&id.machine)
            .copied()
            .ok_or_else(|| shape(id.machine))?;
        loops.push(reconstruct_one(function, component, certificate)?);
    }
    Ok(CountedLoopAnalysisSnapshot {
        revision: unit.identity,
        terminal_psi: unit.psi,
        loops,
    })
}

fn reconstruct_one(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    certificate: &OptimizerUnsignedCountdownRankingCertificate,
) -> Result<UnsignedCountdownLoopSummary, CountedLoopAnalysisError> {
    let preheader_edge = single_edge(&component.entries, function.machine)?;
    let exit_edge = single_edge(&component.exits, function.machine)?;
    let blocks = function
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let preheader = blocks
        .get(&preheader_edge.source)
        .and_then(|block| block.nodes.last())
        .ok_or_else(|| shape(function.machine))?;
    let O::Jump {
        psi_edge,
        target,
        bindings,
        ..
    } = &preheader.operation
    else {
        return Err(shape(function.machine));
    };
    if (*psi_edge, *target) != (preheader_edge.edge, certificate.header)
        || preheader_edge.target != certificate.header
    {
        return Err(shape(function.machine));
    }
    let rank_bindings = bindings
        .iter()
        .filter(|binding| binding.parameter == certificate.rank_parameter)
        .collect::<Vec<_>>();
    let [binding] = rank_bindings.as_slice() else {
        return Err(shape(function.machine));
    };
    if binding.scalar_type != ScalarType::Integer(certificate.rank_type) {
        return Err(shape(function.machine));
    }
    Ok(UnsignedCountdownLoopSummary {
        certificate: certificate.clone(),
        members: component.members.clone(),
        preheader_edge,
        exit_edge,
        trip_count: ExactUnsignedTripCount {
            initial_value: binding.argument,
            scalar_type: certificate.rank_type,
        },
    })
}

fn single_edge(
    edges: &[CycleComponentEdge],
    machine: MachineId,
) -> Result<CycleComponentEdge, CountedLoopAnalysisError> {
    let [edge] = edges else {
        return Err(shape(machine));
    };
    Ok(*edge)
}

fn shape(machine: MachineId) -> CountedLoopAnalysisError {
    CountedLoopAnalysisError::UnsupportedCountdownShape { machine }
}
