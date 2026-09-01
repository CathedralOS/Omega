//! Optimizer module role: proposal leaf. Direct countdown-summary construction.

use super::*;

pub(super) fn propose(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
) -> Result<CountedLoopAnalysisSnapshot, CountedLoopAnalysisError> {
    validate_roots(unit, custody)?;
    let loop_forest = compute_loop_forest(unit);
    let mut loops = Vec::new();
    for certificate in custody.ranking_certificates().certificates() {
        let component = custody
            .components()
            .iter()
            .find(|component| component.id == certificate.component)
            .ok_or(CountedLoopAnalysisError::CertificateComponentRosterMismatch)?;
        let function = unit
            .functions
            .iter()
            .find(|function| function.machine == certificate.component.machine)
            .ok_or_else(|| shape(certificate.component.machine))?;
        let function_regions = loop_forest
            .functions
            .iter()
            .filter(|(machine, _)| *machine == function.machine)
            .collect::<Vec<_>>();
        let [(_, regions)] = function_regions.as_slice() else {
            return Err(shape(function.machine));
        };
        let matching_regions = regions
            .iter()
            .filter(|region| region.blocks == component.members)
            .collect::<Vec<_>>();
        let [region] = matching_regions.as_slice() else {
            return Err(shape(function.machine));
        };
        if region.header != Some(certificate.header) || region.irreducible {
            return Err(shape(function.machine));
        }
        loops.push(summary(function, component, certificate, region)?);
    }
    loops.sort_by(|left, right| left.certificate.component.cmp(&right.certificate.component));
    Ok(CountedLoopAnalysisSnapshot {
        revision: unit.identity,
        terminal_psi: unit.psi,
        loops,
    })
}

fn summary(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    certificate: &OptimizerUnsignedCountdownRankingCertificate,
    region: &LoopRegion,
) -> Result<UnsignedCountdownLoopSummary, CountedLoopAnalysisError> {
    let [preheader_edge] = component.entries.as_slice() else {
        return Err(shape(function.machine));
    };
    let [exit_edge] = component.exits.as_slice() else {
        return Err(shape(function.machine));
    };
    let source = function
        .blocks
        .iter()
        .find(|block| block.id == preheader_edge.source)
        .and_then(|block| block.nodes.last())
        .ok_or_else(|| shape(function.machine))?;
    let O::Jump {
        psi_edge,
        target,
        bindings,
        ..
    } = &source.operation
    else {
        return Err(shape(function.machine));
    };
    let binding = bindings
        .iter()
        .find(|binding| binding.parameter == certificate.rank_parameter)
        .filter(|binding| binding.scalar_type == ScalarType::Integer(certificate.rank_type))
        .ok_or_else(|| shape(function.machine))?;
    if *psi_edge != preheader_edge.edge
        || *target != certificate.header
        || preheader_edge.target != certificate.header
    {
        return Err(shape(function.machine));
    }
    Ok(UnsignedCountdownLoopSummary {
        certificate: certificate.clone(),
        region: (*region).clone(),
        preheader_edge: *preheader_edge,
        exit_edge: *exit_edge,
        trip_count: ExactUnsignedTripCount {
            initial_value: binding.argument,
            scalar_type: certificate.rank_type,
        },
    })
}

fn validate_roots(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
) -> Result<(), CountedLoopAnalysisError> {
    let recomputed = recompute_psi_optimization_unit_identity(unit);
    if recomputed != unit.identity {
        return Err(CountedLoopAnalysisError::StaleUnitIdentity {
            stored: unit.identity,
            recomputed,
        });
    }
    if unit.psi != custody.terminal_psi()
        || unit.psi != custody.ranking_certificates().snapshot().terminal_psi
    {
        return Err(CountedLoopAnalysisError::TerminalIdentityMismatch);
    }
    if custody.components().len() != custody.ranking_certificates().certificates().len() {
        return Err(CountedLoopAnalysisError::CertificateComponentRosterMismatch);
    }
    Ok(())
}

fn shape(machine: MachineId) -> CountedLoopAnalysisError {
    CountedLoopAnalysisError::UnsupportedCountdownShape { machine }
}
