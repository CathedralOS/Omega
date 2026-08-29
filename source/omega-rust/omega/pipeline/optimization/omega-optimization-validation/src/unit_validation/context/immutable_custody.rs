//! Verified structural attachment plus immutable signature and roster custody.

use super::*;

pub(super) fn attach_verified_structural_context(
    unit: &mut PsiOptimizationUnit,
    module: &psi_terminal::TerminalModule,
) -> Result<(), OptimizationUnitValidationError> {
    unit.structural_domains = module.structural_domains.clone().into();
    unit.services = module.services.clone().into();
    unit.root_service_reach = module.root_service_reach.clone();
    for function in &mut unit.functions {
        let source = module
            .machines
            .iter()
            .find(|machine| machine.id == function.machine)
            .ok_or(OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch)?;
        function.structural_places = source.structural_places.clone();
        function.content_entry_claims = source.content_entry_claims.clone();
        function.verified_contract = Some(source.contract.clone());
        function.evidence_contract_lanes = module
            .evidence_contract_lanes
            .iter()
            .filter(|lane| lane.machine == function.machine)
            .cloned()
            .collect();
    }
    unit.identity = recompute_psi_optimization_unit_identity(unit);
    Ok(())
}

pub(super) fn same_immutable_signature_custody(
    seed: &PsiOptimizationUnit,
    unit: &PsiOptimizationUnit,
) -> bool {
    seed.psi == unit.psi
        && seed.entry == unit.entry
        && seed.structural_types == unit.structural_types
        && structural_domain_catalog_identity(seed.structural_domains.as_ref())
            == structural_domain_catalog_identity(unit.structural_domains.as_ref())
        && seed.services == unit.services
        && seed.boundary_machines == unit.boundary_machines
        && seed.provider_candidates == unit.provider_candidates
        && source_roster_partition_is_exact(seed, unit)
        && unit.functions.iter().all(|unit| {
            seed.functions
                .iter()
                .find(|seed| seed.machine == unit.machine)
                .is_some_and(|seed| {
                    seed.machine == unit.machine
                        && seed.attachment == unit.attachment
                        && seed.parameters == unit.parameters
                        && seed.structural_parameters == unit.structural_parameters
                        && seed.structural_places == unit.structural_places
                        && seed.result == unit.result
                        && seed.entry_claim_declarations == unit.entry_claim_declarations
                        && seed.content_entry_claims == unit.content_entry_claims
                        && seed.verified_contract == unit.verified_contract
                        && seed.evidence_contract_lanes == unit.evidence_contract_lanes
                        && seed.entry_claims == unit.entry_claims
                        && seed.published_service_ceiling == unit.published_service_ceiling
                })
        })
}

fn source_roster_partition_is_exact(
    seed: &PsiOptimizationUnit,
    unit: &PsiOptimizationUnit,
) -> bool {
    if unit
        .pruned_machines
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return false;
    }
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
    if active.len() != unit.functions.len() || active.len() + pruned.len() != seed.functions.len() {
        return false;
    }
    let mut active_order = unit.functions.iter().map(|function| function.machine);
    for (ordinal, source) in seed.functions.iter().enumerate() {
        let ordinal = u32::try_from(ordinal).ok();
        if active.contains(&source.machine) {
            if active_order.next() != Some(source.machine) {
                return false;
            }
        } else if ordinal.and_then(|ordinal| pruned.get(&ordinal).copied()) != Some(source.machine)
        {
            return false;
        }
    }
    active_order.next().is_none()
}
