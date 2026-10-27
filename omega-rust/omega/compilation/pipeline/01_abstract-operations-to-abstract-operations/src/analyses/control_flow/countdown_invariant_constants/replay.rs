//! Optimizer module role: validation leaf. Independently keyed constant reconstruction.

use super::{
    AnalysisRoot, CountdownInvariantConstantAnalysisError,
    CountdownInvariantConstantAnalysisSnapshot, CountdownInvariantConstantRole,
    CountdownInvariantIntegerConstant, IntegerType, IntegerValue, NodeLocation, O, OperationId,
    PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance, ScalarType,
    UnsignedCountdownInvariantConstants, ValidatedCountedLoopAnalysis,
    ValidatedOptimizerCycleComponents, ValueDefinitionSite, ValueId, unsupported,
    validate_analysis_root, validate_locations,
};
use std::collections::BTreeMap;

pub(super) fn reconstruct(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
    counted: &ValidatedCountedLoopAnalysis,
) -> Result<CountdownInvariantConstantAnalysisSnapshot, CountdownInvariantConstantAnalysisError> {
    validate_roots(unit, custody, counted)?;
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
                certificate.rank_type,
                CountdownInvariantConstantRole::PositiveGuardZero,
                certificate.guard.zero_operation,
                certificate.guard.zero,
                IntegerValue::Unsigned(0),
            )?,
            reconstruct_row(
                function,
                certificate.rank_type,
                CountdownInvariantConstantRole::BackedgeDecrementOne,
                certificate.descent.one_operation,
                certificate.descent.one,
                IntegerValue::Unsigned(1),
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

#[allow(clippy::too_many_arguments)]
fn reconstruct_row(
    function: &PsiOptimizationFunction,
    rank_type: IntegerType,
    role: CountdownInvariantConstantRole,
    operation: OperationId,
    result: ValueId,
    value: IntegerValue,
) -> Result<CountdownInvariantIntegerConstant, CountdownInvariantConstantAnalysisError> {
    let mut occurrences = function
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
    let Some((block, node_index, node)) = occurrences.next() else {
        return Err(unsupported(function.machine, operation));
    };
    if occurrences.next().is_some() {
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
        || scalar_type != ScalarType::Integer(rank_type)
        || actual_value != value
        || definition.value != result
        || definition.scalar_type != ScalarType::Integer(rank_type)
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
        scalar_type: rank_type,
        value,
        definition: *definition,
        provenance: node.provenance.clone(),
        fuel: node.fuel.clone(),
        effect: node.effect,
    })
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
