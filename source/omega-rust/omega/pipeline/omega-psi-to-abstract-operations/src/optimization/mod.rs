//! Optimizer-input entrance: reconstruct the complete unit seed, bind admitted
//! proof facts, proof questions, and ownership frontiers, then seal the unit
//! beside the verifier-owned input context.

mod accepted_obligations;
mod error;
mod model;
mod ownership_frontiers;
mod proof_questions;

pub use error::VerifiedPsiOptimizationUnitBuildError;
pub use model::{
    VerifiedPsiOptimizationContext, VerifiedPsiOptimizationInput, VerifiedPsiOptimizationUnit,
};

use accepted_obligations::project_accepted_obligation_facts;
use ownership_frontiers::project_ownership_frontiers;
use proof_questions::project_proof_questions;

/// The only optimizer-facing unit constructor. Consuming the verified carrier
/// prevents callers from pairing a plan with evidence admitted for a different
/// Terminal-Psi artifact.
pub fn build_verified_psi_optimization_unit(
    input: VerifiedPsiOptimizationInput,
    fuel_schedule: psi_core::FuelScheduleIdentity,
) -> Result<VerifiedPsiOptimizationUnit, VerifiedPsiOptimizationUnitBuildError> {
    let mut seed = omega_optimization_unit::reconstruct_psi_optimization_unit_seed(
        input.plan(),
        fuel_schedule,
    )?;
    let context = input.context();
    seed.structural_domains = context.module().structural_domains.clone().into();
    seed.services = context.module().services.clone().into();
    seed.root_service_reach = context.module().root_service_reach.clone();
    for function in &mut seed.functions {
        let source = context
            .module()
            .machines
            .iter()
            .find(|machine| machine.id == function.machine)
            .ok_or(
                VerifiedPsiOptimizationUnitBuildError::MissingStructuralCatalogMachine(
                    function.machine,
                ),
            )?;
        function.structural_places = source.structural_places.clone();
        function.content_entry_claims = source.content_entry_claims.clone();
        function.verified_contract = Some(source.contract.clone());
        function.evidence_contract_lanes = context
            .module()
            .evidence_contract_lanes
            .iter()
            .filter(|lane| lane.machine == function.machine)
            .cloned()
            .collect();
    }
    seed.identity = omega_optimization_unit::recompute_psi_optimization_unit_identity(&seed);
    let facts = project_accepted_obligation_facts(&seed, context)?;
    let unit = omega_optimization_unit::attach_accepted_obligation_facts(seed, facts)?;
    let proof_questions = project_proof_questions(&input)?;
    let unit = omega_optimization_unit::attach_proof_questions(unit, proof_questions)?;
    let ownership_frontiers = project_ownership_frontiers(&input)?;
    let unit = omega_optimization_unit::attach_ownership_frontier_facts(unit, ownership_frontiers)?;
    Ok(VerifiedPsiOptimizationUnit { input, unit })
}
