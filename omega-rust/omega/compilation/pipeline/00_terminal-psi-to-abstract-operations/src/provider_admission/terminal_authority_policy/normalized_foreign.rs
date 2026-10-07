//! Optimizer module role: construction leaf. Canonical normalized-foreign mechanism identity.

use effects::TerminalMechanismIdentity;

/// Reconstruct the exact normalized-foreign role from the retained locator and
/// canonical admitted boundary plan. Its strong contract digest enters identity.
pub fn normalized_foreign_terminal_mechanism(
    locator: &target::NormalizedForeignLocator,
    boundary_entry_plan: &calling_conventions::BoundaryEntryPlan,
) -> Result<TerminalMechanismIdentity, String> {
    let signature = entry_call_signature(boundary_entry_plan);
    let validated =
        calling_conventions::validate_boundary_entry_plan(boundary_entry_plan.clone(), &signature)
            .map_err(|diagnostic| diagnostic.to_string())?;
    require_canonical_plan(
        &validated,
        boundary_entry_plan,
        "normalized foreign boundary plan is not canonical",
    )?;
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
    let signature = entry_call_signature(boundary_entry_plan);
    let validated =
        calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
            boundary_entry_plan.clone(),
            &signature,
            context,
        )
        .map_err(|diagnostic| diagnostic.to_string())?;
    require_canonical_plan(
        &validated,
        boundary_entry_plan,
        "normalized foreign callback boundary plan is not canonical",
    )?;
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

/// The boundary entry plan's call signature restated as a
/// `calling_conventions::CallSignature`.
fn entry_call_signature(
    boundary_entry_plan: &calling_conventions::BoundaryEntryPlan,
) -> calling_conventions::CallSignature {
    calling_conventions::CallSignature {
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
    }
}

/// Reject a revalidated plan whose canonicalization rewrote the entry plan.
fn require_canonical_plan(
    validated: &calling_conventions::ValidatedBoundaryEntryPlan,
    boundary_entry_plan: &calling_conventions::BoundaryEntryPlan,
    reason: &str,
) -> Result<(), String> {
    if validated.plan() != boundary_entry_plan {
        return Err(reason.to_owned());
    }
    Ok(())
}
