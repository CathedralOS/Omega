//! Optimizer module role: construction leaf. Canonical normalized-foreign mechanism identity.

use effects::TerminalMechanismIdentity;

/// Reconstruct the exact normalized-foreign role from the retained locator and
/// canonical admitted boundary plan. Its strong contract digest enters identity.
pub fn normalized_foreign_terminal_mechanism(
    locator: &target::NormalizedForeignLocator,
    boundary_entry_plan: &calling_conventions::BoundaryEntryPlan,
) -> Result<TerminalMechanismIdentity, String> {
    let signature = calling_conventions::CallSignature {
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
    let validated =
        calling_conventions::validate_boundary_entry_plan(boundary_entry_plan.clone(), &signature)
            .map_err(|diagnostic| diagnostic.to_string())?;
    if validated.plan() != boundary_entry_plan {
        return Err("normalized foreign boundary plan is not canonical".to_owned());
    }
    Ok(
        effects::NormalizedForeignTerminalMechanismIdentity::from_normalized_locator(
            locator,
            effects::provider_plan::BoundaryCallingPlanCommitment::from_digest(
                validated.contract_commitment_digest(),
            ),
        )
        .into(),
    )
}

/// Reconstruct one normalized-foreign role whose exact outbound contract
/// includes compiler-private callback materialization. The context is required
/// because the plan alone cannot authenticate nominal binders or destinations.
pub fn normalized_foreign_terminal_mechanism_with_callback_materializations(
    locator: &target::NormalizedForeignLocator,
    boundary_entry_plan: &calling_conventions::BoundaryEntryPlan,
    context: &calling_conventions::CallbackMaterializationContext,
) -> Result<TerminalMechanismIdentity, String> {
    let signature = calling_conventions::CallSignature {
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
    let validated =
        calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
            boundary_entry_plan.clone(),
            &signature,
            context,
        )
        .map_err(|diagnostic| diagnostic.to_string())?;
    if validated.plan() != boundary_entry_plan {
        return Err("normalized foreign callback boundary plan is not canonical".to_owned());
    }
    Ok(
        effects::NormalizedForeignTerminalMechanismIdentity::from_normalized_locator(
            locator,
            effects::provider_plan::BoundaryCallingPlanCommitment::from_digest(
                validated.contract_commitment_digest(),
            ),
        )
        .into(),
    )
}
