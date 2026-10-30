//! Optimizer module role: validation leaf. Independently keyed constant reconstruction.

use super::{
    AnalysisRoot, CountdownInvariantConstantAnalysisError,
    CountdownInvariantConstantAnalysisSnapshot, CountdownInvariantConstantRole,
    CountdownInvariantIntegerConstant, OptimizerUnsignedCountdownRankingCertificate,
    PsiOptimizationFunction, PsiOptimizationUnit, UnsignedCountdownInvariantConstants,
    ValidatedCountedLoopAnalysis, ValidatedOptimizerCycleComponents, by_key, expected_constant,
    invariant_constant_row, sole_provenance_node, unsupported, validate_analysis_root,
    validate_locations,
};

pub(super) fn reconstruct(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
    counted: &ValidatedCountedLoopAnalysis,
) -> Result<CountdownInvariantConstantAnalysisSnapshot, CountdownInvariantConstantAnalysisError> {
    validate_roots(unit, custody, counted)?;
    let functions = by_key(&unit.functions, |function| function.machine);
    let components = by_key(custody.components(), |component| component.id.clone());
    let counted_loops = by_key(counted.loops(), |summary| {
        summary.certificate.component.clone()
    });
    if functions.len() != unit.functions.len()
        || components.len() != custody.components().len()
        || counted_loops.len() != counted.loops().len()
    {
        return Err(CountdownInvariantConstantAnalysisError::ComponentRosterMismatch);
    }

    let mut loops = Vec::with_capacity(custody.ranking_certificates().certificates().len());
    for certificate in custody.ranking_certificates().certificates() {
        let component = components
            .get(&certificate.component)
            .copied()
            .ok_or(CountdownInvariantConstantAnalysisError::ComponentRosterMismatch)?;
        let summary = counted_loops
            .get(&certificate.component)
            .copied()
            .filter(|summary| summary.certificate == *certificate)
            .ok_or(CountdownInvariantConstantAnalysisError::ComponentRosterMismatch)?;
        let function = functions
            .get(&certificate.component.machine)
            .copied()
            .ok_or(CountdownInvariantConstantAnalysisError::ComponentRosterMismatch)?;
        if summary.region.blocks != component.members
            || summary.preheader_edge.target != certificate.header
        {
            return Err(CountdownInvariantConstantAnalysisError::ComponentRosterMismatch);
        }
        let constants = [
            reconstruct_row(
                function,
                certificate,
                CountdownInvariantConstantRole::PositiveGuardZero,
            )?,
            reconstruct_row(
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
        root: AnalysisRoot::of(unit),
        loops,
    })
}

/// The certified occurrence the keyed path admits: the unique node whose
/// provenance names the role's operation. The proposal leaf reaches the same
/// node through the operation shape instead.
fn reconstruct_row(
    function: &PsiOptimizationFunction,
    certificate: &OptimizerUnsignedCountdownRankingCertificate,
    role: CountdownInvariantConstantRole,
) -> Result<CountdownInvariantIntegerConstant, CountdownInvariantConstantAnalysisError> {
    let expected = expected_constant(certificate, role);
    let (block, node_index, node) =
        sole_provenance_node(function.blocks.iter(), expected.operation)
            .ok_or_else(|| unsupported(function.machine, expected.operation))?;
    invariant_constant_row(function, block, node_index, node, role, expected)
        .ok_or_else(|| unsupported(function.machine, expected.operation))
}

fn validate_roots(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
    counted: &ValidatedCountedLoopAnalysis,
) -> Result<(), CountdownInvariantConstantAnalysisError> {
    validate_analysis_root(unit, custody, &[counted.snapshot().root])
        .map_err(CountdownInvariantConstantAnalysisError::Root)?;
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
    // Certified components are the subset of the validated SCC roster named by
    // ranking certificates; the counted roster must cover exactly that subset.
    if certificate_keys != counted_keys
        || certificate_keys
            .iter()
            .any(|key| !component_keys.contains(key))
    {
        return Err(CountdownInvariantConstantAnalysisError::ComponentRosterMismatch);
    }
    Ok(())
}
