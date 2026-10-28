//! Optimizer module role: proposal leaf. Direct countdown-summary construction.

use super::{
    AnalysisRoot, CountedLoopAnalysisError, CountedLoopAnalysisSnapshot, ExactUnsignedTripCount,
    LoopRegion, O, OptimizerCycleComponent, OptimizerUnsignedCountdownRankingCertificate,
    PsiOptimizationFunction, PsiOptimizationUnit, ScalarType, UnsignedCountdownLoopSummary,
    ValidatedOptimizerCycleComponents, region, shape, validate_analysis_root,
};
pub(super) fn propose(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
) -> Result<CountedLoopAnalysisSnapshot, CountedLoopAnalysisError> {
    validate_roots(unit, custody)?;
    let mut loops = Vec::new();
    for certificate in custody.ranking_certificates().certificates() {
        let component = custody
            .components()
            .iter()
            .find(|component| component.id == certificate.component)
            .ok_or(CountedLoopAnalysisError::CertificateComponentRosterMismatch)?;
        let function = unit
            .function(certificate.component.machine)
            .ok_or_else(|| shape(certificate.component.machine))?;
        // The region is projected from the validated Terminal-SCC custody the
        // certificate keys on, not from a private loop-forest re-derivation.
        let region = region::derive(function.machine, component, certificate)?;
        loops.push(summary(function, component, certificate, &region)?);
    }
    loops.sort_by(|left, right| left.certificate.component.cmp(&right.certificate.component));
    Ok(CountedLoopAnalysisSnapshot {
        root: AnalysisRoot::of(unit),
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
        .block(preheader_edge.source)
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
    validate_analysis_root(unit, custody, &[]).map_err(CountedLoopAnalysisError::Root)?;
    // Ranking certificates name the certified subset of the validated SCC
    // roster; uncertified Natural and unranked components remain in custody
    // without entering this analysis.
    if custody
        .ranking_certificates()
        .certificates()
        .iter()
        .any(|certificate| {
            !custody
                .components()
                .iter()
                .any(|component| component.id == certificate.component)
        })
    {
        return Err(CountedLoopAnalysisError::CertificateComponentRosterMismatch);
    }
    Ok(())
}
