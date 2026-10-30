//! Optimizer module role: proposal leaf. Direct certificate-owned constant construction.

use super::{
    AnalysisRoot, CountdownInvariantConstantAnalysisError,
    CountdownInvariantConstantAnalysisSnapshot, CountdownInvariantConstantRole,
    CountdownInvariantIntegerConstant, O, OptimizerUnsignedCountdownRankingCertificate,
    PsiOptimizationFunction, PsiOptimizationUnit, UnsignedCountdownInvariantConstants,
    ValidatedCountedLoopAnalysis, ValidatedOptimizerCycleComponents, expected_constant,
    invariant_constant_row, validate_analysis_root, validate_locations,
};
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
            .function(certificate.component.machine)
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
        root: AnalysisRoot::of(unit),
        loops,
    })
}

/// The certified occurrence the operation shape admits: exactly one
/// `IntegerConstant` in the function carries the role's operation. The
/// acceptance replay reaches the same node through provenance instead, so the
/// two sides keep independent discoveries of one row.
fn locate(
    function: &PsiOptimizationFunction,
    certificate: &OptimizerUnsignedCountdownRankingCertificate,
    role: CountdownInvariantConstantRole,
) -> Result<CountdownInvariantIntegerConstant, CountdownInvariantConstantAnalysisError> {
    let expected = expected_constant(certificate, role);
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
            matches!(
                node.operation,
                O::IntegerConstant { psi_operation, .. } if psi_operation == expected.operation
            )
        });
    let Some((block, node_index, node)) = matches.next() else {
        return Err(unsupported(function.machine, expected.operation));
    };
    if matches.next().is_some() {
        return Err(unsupported(function.machine, expected.operation));
    }
    let node_index =
        u32::try_from(node_index).map_err(|_| unsupported(function.machine, expected.operation))?;
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
    // The counted roster covers exactly the certified subset of the validated
    // SCC roster; uncertified components carry no countdown custody.
    if custody.ranking_certificates().certificates().len() != counted.loops().len() {
        return Err(CountdownInvariantConstantAnalysisError::ComponentRosterMismatch);
    }
    Ok(())
}

fn unsupported(
    machine: semantic_vocabulary::MachineId,
    operation: semantic_vocabulary::OperationId,
) -> CountdownInvariantConstantAnalysisError {
    CountdownInvariantConstantAnalysisError::UnsupportedInvariantConstant { machine, operation }
}
