//! Project checked product custody into source-free native compilation inputs.

use crate::checked::CheckedCompilation;
use diagnostics::Diagnostic;

/// Borrow one complete production input per explicitly Independent selected
/// plan. A description with no current native product is a refusal here, never
/// a reason to realize the same body inside the receiver.
pub(super) fn independent_components<'input>(
    checked: &'input CheckedCompilation,
    terminal: &compilation_products::RetainedTerminalArtifact,
) -> Result<Vec<native_production::ComponentCompilation<'input>>, Vec<Diagnostic>> {
    let proposal = terminal.native_realization_proposal().ok_or_else(|| {
        vec![Diagnostic::error(
            "native product lost its retained realization proposal",
        )]
    })?;
    let mut result = Vec::new();
    for selected in &checked.execution().selected_provider_provenance {
        if selected
            .selected_by
            .composition_mode()
            .map_err(|reason| vec![Diagnostic::error(reason)])?
            != provider_planning::CompositionMode::Independent
        {
            continue;
        }
        let mut components = checked
            .providers()
            .verified_independent_components
            .iter()
            .filter(|component| component.realizes_selected_plan(&selected.plan).is_ok());
        let component = components.next().ok_or_else(|| {
            vec![Diagnostic::error(
                "independent plan has no exact verified producing component",
            )]
        })?;
        if components.next().is_some() {
            return Err(vec![Diagnostic::error(
                "independent plan has competing producing component verdicts",
            )]);
        }
        let mut products = checked
            .independent_component_products()
            .iter()
            .filter(|product| {
                product.description().expected_subject() == component.subject()
                    && product
                        .retained()
                        .native_realization_proposal()
                        .is_some_and(|candidate| {
                            candidate.target_profile() == proposal.target_profile()
                        })
            });
        let product = products.next().ok_or_else(|| {
            vec![Diagnostic::error(
                "independent plan has no exact retained native component product",
            )]
        })?;
        if products.next().is_some() {
            return Err(vec![Diagnostic::error(
                "independent plan has competing retained component products",
            )]);
        }
        let input = native_production::ComponentNativeInputs::join(
            component,
            product.retained(),
            &selected.plan,
            &proposal.boundary_applications().calling,
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        // Keep the canonical receiving plan beside this component, rather than
        // recovering it separately for every emitted export. This is selection
        // custody only; native admission still checks each actual image/call.
        let facts = &checked.execution().selected_provider_plan_facts;
        let digest = selected.plan.identity_digest();
        let mut plans = facts
            .plans()
            .iter()
            .filter(|plan| plan.identity_digest() == digest);
        let plan = plans.next().ok_or_else(|| {
            vec![Diagnostic::error(
                "component call lost its complete selected provider plan",
            )]
        })?;
        if plans.next().is_some() {
            return Err(vec![Diagnostic::error(
                "component call has competing complete provider plans",
            )]);
        }
        result.push(native_production::ComponentCompilation {
            input,
            plan,
            receiving_identities: facts.identities(),
            accepted_package_permissions: product.accepted_package_permissions(),
        });
    }
    Ok(result)
}

/// Project and validate the checked package's accepted Terminal permission
/// rows before native proposal production. These are package evidence, not
/// the independently supplied receiver-admission policy.
pub(crate) fn accepted_terminal_authority_permission_policy(
    checked: &CheckedCompilation,
) -> Result<
    terminal_psi_to_abstract_operations::provider_admission::TerminalAuthorityPermissionPolicy,
    Vec<Diagnostic>,
> {
    use terminal_psi_to_abstract_operations::provider_admission::{
        terminal_authority_permission_policy_with_rows,
        terminal_authority_permissions::validate_package_terminal_authority_permission_custody,
    };
    let (permissions, permission_identities) = checked.execution().terminal_authority_permissions();
    validate_package_terminal_authority_permission_custody(
        permissions.iter(),
        &permission_identities,
    )?;
    terminal_authority_permission_policy_with_rows(permissions, permission_identities).map_err(
        |error| {
            vec![Diagnostic::error(format!(
                "checked package terminal-authority permissions are invalid: {error:?}"
            ))]
        },
    )
}
