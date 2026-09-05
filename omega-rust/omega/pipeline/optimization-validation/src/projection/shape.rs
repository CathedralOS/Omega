//! Projected-plan shape, source roster, and reconstructible equivalence.

use super::*;

pub(super) fn validate_projection_shape(
    source: &AbstractOperationPlan,
    final_unit: &PsiOptimizationUnit,
    projected: &AbstractOperationPlan,
) -> Result<(), OptimizedAbstractPlanProjectionError> {
    if projected.psi != source.psi
        || final_unit.psi != source.psi
        || final_unit.entry != source.entry
        || projected.entry != final_unit.entry
        || final_unit.structural_types != source.structural_types
        || projected.structural_types != source.structural_types
        || final_unit.boundary_machines != source.boundary_machines
        || projected.boundary_machines != source.boundary_machines
        || final_unit.provider_candidates != source.provider_candidates
        || projected.provider_candidates != source.provider_candidates
    {
        return Err(OptimizedAbstractPlanProjectionError::ImmutablePlanMetadataMismatch);
    }
    if !source_function_roster_partition_is_exact(source, final_unit)
        || projected.functions.len() != final_unit.functions.len()
        || projected
            .functions
            .iter()
            .map(|function| function.machine)
            .ne(final_unit.functions.iter().map(|function| function.machine))
    {
        return Err(OptimizedAbstractPlanProjectionError::SourceFunctionRosterMismatch);
    }
    for (unit_function, projected_function) in final_unit.functions.iter().zip(&projected.functions)
    {
        let Some(source_function) = source
            .functions
            .iter()
            .find(|source| source.machine == unit_function.machine)
        else {
            return Err(OptimizedAbstractPlanProjectionError::SourceFunctionRosterMismatch);
        };
        if projected_function.attachment != source_function.attachment
            || unit_function.attachment != source_function.attachment
            || projected_function.structural_parameters != source_function.structural_parameters
            || unit_function.structural_parameters != source_function.structural_parameters
            || projected_function.result != source_function.result
            || unit_function.result != source_function.result
            || projected_function.entry_claims != source_function.entry_claims
            || unit_function.entry_claim_declarations != source_function.entry_claims
            || projected_function.published_service_ceiling
                != source_function.published_service_ceiling
            || unit_function.published_service_ceiling != source_function.published_service_ceiling
            || unit_function.entry_claims
                != source_function
                    .entry_claims
                    .iter()
                    .map(|claim| claim.claim)
                    .collect()
        {
            return Err(OptimizedAbstractPlanProjectionError::ImmutablePlanMetadataMismatch);
        }
    }

    let reconstructed = optimization_unit::reconstruct_psi_optimization_unit_seed(
        projected,
        final_unit.fuel_schedule,
    )
    .map_err(|_| OptimizedAbstractPlanProjectionError::ReconstructibleProjectionMismatch)?;
    if !same_reconstructible_projection(&reconstructed, final_unit) {
        return Err(OptimizedAbstractPlanProjectionError::ReconstructibleProjectionMismatch);
    }
    Ok(())
}

fn source_function_roster_partition_is_exact(
    source: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> bool {
    let active = unit
        .functions
        .iter()
        .map(|function| function.machine)
        .collect::<BTreeSet<_>>();
    let pruned = unit
        .pruned_machines
        .iter()
        .map(|row| (row.source_ordinal, row.machine))
        .collect::<BTreeMap<_, _>>();
    if unit
        .pruned_machines
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
        || active.len() != unit.functions.len()
        || active.len() + pruned.len() != source.functions.len()
    {
        return false;
    }
    let mut active_order = unit.functions.iter().map(|function| function.machine);
    for (ordinal, source_function) in source.functions.iter().enumerate() {
        if active.contains(&source_function.machine) {
            if active_order.next() != Some(source_function.machine) {
                return false;
            }
        } else if u32::try_from(ordinal)
            .ok()
            .and_then(|ordinal| pruned.get(&ordinal).copied())
            != Some(source_function.machine)
        {
            return false;
        }
    }
    active_order.next().is_none()
}

fn same_reconstructible_projection(
    reconstructed: &PsiOptimizationUnit,
    final_unit: &PsiOptimizationUnit,
) -> bool {
    reconstructed.psi == final_unit.psi
        && reconstructed.fuel_schedule == final_unit.fuel_schedule
        && reconstructed.entry == final_unit.entry
        && reconstructed.structural_types == final_unit.structural_types
        && reconstructed.boundary_machines == final_unit.boundary_machines
        && reconstructed.provider_candidates == final_unit.provider_candidates
        && reconstructed.functions.len() == final_unit.functions.len()
        && reconstructed
            .functions
            .iter()
            .zip(&final_unit.functions)
            .all(|(left, right)| {
                left.machine == right.machine
                    && left.attachment == right.attachment
                    && left.entry == right.entry
                    && left.parameters == right.parameters
                    && left.structural_parameters == right.structural_parameters
                    && left.result == right.result
                    && left.declared_places == right.declared_places
                    && left.entry_claim_declarations == right.entry_claim_declarations
                    && left.entry_claims == right.entry_claims
                    && left.published_service_ceiling == right.published_service_ceiling
                    && left.facts == right.facts
                    && left.blocks.len() == right.blocks.len()
                    && left.blocks.iter().zip(&right.blocks).all(|(left, right)| {
                        left.id == right.id
                            && left.parameters == right.parameters
                            && left.nodes.len() == right.nodes.len()
                            && left.nodes.iter().zip(&right.nodes).all(|(left, right)| {
                                left.operation == right.operation
                                    && left.effect == right.effect
                                    && left.definitions == right.definitions
                                    && left.uses == right.uses
                                    && left.successors.len() == right.successors.len()
                                    && left.successors.iter().zip(&right.successors).all(
                                        |(left, right)| {
                                            left.psi_edge == right.psi_edge
                                                && left.target == right.target
                                                && left.bindings == right.bindings
                                        },
                                    )
                                    && left.ownership == right.ownership
                            })
                    })
            })
}
