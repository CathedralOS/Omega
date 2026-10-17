//! Project complete closed source applications through selected Import custody.
//!
//! A foreign realization has no checked body to specialize. Its selected
//! provenance already owns the exact requirement and realization declaration;
//! each closed source application supplies a distinct coordinate of that same
//! binding. This projection preserves the source tuple, including unused types
//! and machine operands, rather than deriving a tuple from ABI equality.

use super::SelectedProviderReviewProvenance;
use checked_trees::typed_trees::{TypedTrees, trait_definition::AppliedTraitScope};
use effects::provider_plan::ProviderBinding;
use effects::{SelectedExternalRequirementApplication, SelectedProviderPlanFacts};

pub(super) fn derive(
    typed: &TypedTrees,
    selected: &SelectedProviderPlanFacts,
    provenance: &[SelectedProviderReviewProvenance],
) -> Result<
    (
        Vec<SelectedExternalRequirementApplication>,
        terminal_psi::TerminalIdentityTable,
    ),
    String,
> {
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let mut rows = Vec::new();
    for plan in selected.plans() {
        if !plan
            .rows
            .iter()
            .any(|row| matches!(row.binding, ProviderBinding::Import { .. }))
        {
            continue;
        }
        // Ordinary and toolchain-settled imports need no source application
        // roster. Demand this provenance only for a retained closed coordinate
        // of an exact template row, never every foreign leaf in the closure.
        if !typed.closed_requirement_applications().any(|application| {
            typed
                .trait_machine_signature(application.template)
                .is_some_and(|(owner, signature)| {
                    let identity = typed.trait_requirement_callable_identity_text(owner, signature);
                    plan.rows.iter().any(|row| {
                        matches!(row.binding, ProviderBinding::Import { .. })
                            && plan.identities.text(row.requirement_identity) == identity
                    })
                })
        }) {
            continue;
        }
        let mut retained = provenance.iter().filter(|retained| retained.plan == *plan);
        let retained = retained
            .next()
            .filter(|_| retained.next().is_none())
            .ok_or("selected Import plan lost its unique exact declaration provenance")?;
        if retained.provider.row_requirements.len() != plan.rows.len()
            || retained.provider.row_realizations.len() != plan.rows.len()
        {
            return Err("selected Import provenance lost its row alignment".into());
        }
        for (index, row) in plan.rows.iter().enumerate() {
            if !matches!(row.binding, ProviderBinding::Import { .. }) {
                continue;
            }
            let template = retained.provider.row_requirements[index];
            let Some((owner, signature)) = typed.trait_machine_signature(template) else {
                continue;
            };
            if typed.trait_requirement_callable_identity_text(owner, signature)
                != plan.identities.text(row.requirement_identity)
            {
                return Err(
                    "selected Import template identity disagrees with its provenance".into(),
                );
            }
            let realization = typed
                .machine(retained.provider.row_realizations[index])
                .ok_or("selected Import provenance lost its realization declaration")?;
            let scope = if let Some(declaration) = retained
                .provider
                .provider_type
                .and_then(|symbol| typed.conformances().iter().find(|map| map.symbol == symbol))
            {
                if declaration
                    .rows
                    .iter()
                    .filter(|entry| {
                        entry.requirement == template
                            && entry.declaring_trait == owner.symbol
                            && crate::target_machines::realized_row_machine(
                                crate::ProviderPlanningContext::new(
                                    typed,
                                    source::DependencyScope::Product,
                                ),
                                entry.realization_machine,
                                Some(&plan.target),
                            )
                            .is_some_and(|machine| machine.symbol == realization.symbol)
                    })
                    .count()
                    != 1
                {
                    return Err("selected Import lost its exact named implementation row".into());
                }
                AppliedTraitScope::from_conformance(typed, declaration)
            } else {
                let mut edges =
                    typed
                        .machine_trait_conformances(realization)
                        .iter()
                        .filter(|edge| {
                            edge.requirement_symbol == template && edge.symbol == owner.symbol
                        });
                let edge = edges
                    .next()
                    .filter(|_| edges.next().is_none())
                    .ok_or("selected Import lost its unique external satisfaction")?;
                AppliedTraitScope::from_machine_conformance(typed, realization, edge)
            }
            .ok_or("selected Import lost its complete declaring application")?;
            // Source checking admitted the complete generic conformance and all
            // actual operands. There is no specialized executable tuple here:
            // retain the closed application's own exact identity and outer scope.
            for application in typed
                .closed_requirement_applications()
                .filter(|application| {
                    application.template == template
                        && typed.requirement_application_matches_scope(
                            application.signature.symbol,
                            &scope,
                        )
                })
            {
                let required = typed.state_signature_type_parameters(signature);
                let actual = typed.machine_type_parameters(realization);
                if required.len() != actual.len()
                    || application.local_arguments.len() != required.len()
                    || required.iter().zip(actual).any(|(required, actual)| {
                        std::mem::discriminant(&required.kind)
                            != std::mem::discriminant(&actual.kind)
                    })
                {
                    return Err("selected Import closed application lost its complete declaration telescope".into());
                }
                rows.push(SelectedExternalRequirementApplication {
                    provider_plan_report_identity: plan.report_fingerprint(),
                    provider_plan_digest: plan.identity_digest(),
                    selected_requirement_identity: identities
                        .intern(plan.identities.text(row.requirement_identity)),
                    requirement_identity: identities.intern(
                        &typed
                            .trait_requirement_callable_identity_text(owner, application.signature),
                    ),
                });
            }
        }
    }
    Ok((rows, identities.build()))
}
