//! Optimizer module role: construction leaf. Canonical normalized-foreign mechanism identity.

use omega_effects::TerminalMechanismIdentity;

/// Reconstruct the exact normalized-foreign role from the retained locator and
/// canonical admitted boundary plan. Its strong contract digest enters identity.
pub fn normalized_foreign_terminal_mechanism(
    locator: &omega_target::NormalizedForeignLocator,
    boundary_entry_plan: &omega_calling_conventions::BoundaryEntryPlan,
) -> Result<TerminalMechanismIdentity, String> {
    let signature = omega_calling_conventions::CallSignature {
        parameters: boundary_entry_plan
            .call
            .parameters
            .iter()
            .map(|placement| placement.shape)
            .collect(),
        result: boundary_entry_plan
            .call
            .result
            .as_ref()
            .map(|placement| placement.shape),
    };
    let validated = omega_calling_conventions::validate_boundary_entry_plan(
        boundary_entry_plan.clone(),
        &signature,
    )
    .map_err(|diagnostic| diagnostic.to_string())?;
    if validated.plan() != boundary_entry_plan {
        return Err("normalized foreign boundary plan is not canonical".to_owned());
    }
    Ok(
        omega_effects::NormalizedForeignTerminalMechanismIdentity::from_normalized_locator(
            locator,
            omega_effects::provider_plan::BoundaryCallingPlanCommitment::from_digest(
                validated.contract_commitment_digest(),
            ),
        )
        .into(),
    )
}
