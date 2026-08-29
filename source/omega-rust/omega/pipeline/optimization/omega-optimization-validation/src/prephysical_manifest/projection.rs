use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn expected_record(
    input: &VerifiedPsiOptimizationInput,
    final_unit: &PsiOptimizationUnit,
    selections: &OptimizationSelections,
    psi_selections: &OptimizationSelections,
    budget_per_pass: OptimizationWorkBudget,
    usage: OptimizationWorkUsage,
    decisions: &BaselineDecisionLog,
    pass_manifests: &[OptimizationPassManifestRecord],
    ledger: &PsiTransformationLedger,
    bundle: OptimizationIdentityBundle,
    projection: ValidatedOptimizedAbstractPlanProjection,
) -> Result<PrePhysicalOptimizationManifest, PrePhysicalOptimizationManifestError> {
    let initial = omega_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input.clone(),
        final_unit.fuel_schedule,
    )
    .map_err(|_| PrePhysicalOptimizationManifestError::InitialUnitProjection)?;
    Ok(PrePhysicalOptimizationManifest {
        identity: PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(b"pending"),
        stage: OptimizationManifestStage::PrePhysicalAbstractPlan,
        physical_data: PhysicalOptimizationDataStatus::UnavailableBeforePhysicalRealization,
        psi: final_unit.psi,
        fuel_schedule: final_unit.fuel_schedule,
        initial_unit: initial.unit().identity,
        final_unit: final_unit.identity,
        projection: projection.identity(),
        selections: selections.clone(),
        psi_selections: psi_selections.clone(),
        budget_per_pass,
        usage,
        decision_log: decisions.clone(),
        pass_manifests: pass_manifests.to_vec(),
        transformation_ledger: ledger.clone(),
        identity_bundle: bundle,
        source_statistics: structural_statistics(initial.unit())?,
        optimized_statistics: structural_statistics(final_unit)?,
    })
}

fn structural_statistics(
    unit: &PsiOptimizationUnit,
) -> Result<OptimizationStructuralStatistics, PrePhysicalOptimizationManifestError> {
    let count = |value: usize| {
        u64::try_from(value)
            .map_err(|_| PrePhysicalOptimizationManifestError::StructuralStatisticsOverflow)
    };
    Ok(OptimizationStructuralStatistics {
        functions: count(unit.functions.len())?,
        blocks: count(
            unit.functions
                .iter()
                .map(|function| function.blocks.len())
                .sum(),
        )?,
        nodes: count(
            unit.functions
                .iter()
                .flat_map(|function| &function.blocks)
                .map(|block| block.nodes.len())
                .sum(),
        )?,
        scalar_definitions: count(
            unit.functions
                .iter()
                .map(|function| {
                    function.parameters.len()
                        + function
                            .blocks
                            .iter()
                            .map(|block| {
                                block.parameters.len()
                                    + block
                                        .nodes
                                        .iter()
                                        .map(|node| node.definitions.len())
                                        .sum::<usize>()
                            })
                            .sum::<usize>()
                })
                .sum(),
        )?,
        scalar_uses: count(
            unit.functions
                .iter()
                .flat_map(|function| &function.blocks)
                .flat_map(|block| &block.nodes)
                .map(|node| node.uses.len())
                .sum(),
        )?,
        optimization_facts: count(
            unit.functions
                .iter()
                .map(|function| function.facts.len())
                .sum(),
        )?,
        ownership_frontier_facts: count(unit.ownership_frontier_facts.len())?,
    })
}
