//! Bind selected plans, admit their policy, then enrich their complete closure.
//!
//! Psi has already projected exact declaration associations. This operation
//! owns selected-plan policy and phase order without any source-tree input.

use diagnostics::Diagnostic;
use effects::provider_grants::{AuthoredRootGrant, ResolvedAuthoredSelectedProviderGrant};
use effects::provider_plan::{ProviderBinding, ProviderPlan};
use language_semantics::provider_declarations::ProviderDeclarations;

/// Ephemeral declaration view bound to one selected plan. Preparation errors
/// remain data until the original installation-reach phase consumes them.
pub struct SelectedPlanDeclarations<'plan> {
    pub plan: &'plan ProviderPlan,
    pub declarations: ProviderDeclarations,
}

#[derive(Debug)]
pub struct SelectedProviderPlanBinding {
    selected: effects::SelectedProviderPlanFacts,
    grants: Vec<ResolvedAuthoredSelectedProviderGrant>,
}

impl SelectedProviderPlanBinding {
    pub fn into_parts(
        self,
    ) -> (
        effects::SelectedProviderPlanFacts,
        Vec<ResolvedAuthoredSelectedProviderGrant>,
    ) {
        (self.selected, self.grants)
    }
}

/// Grants and qualification precede boundary policy; reach, physical aliases,
/// then Import applications preserve their independent diagnostic phases.
pub fn bind_selected_provider_plan_facts(
    boundary_admission: crate::BoundaryProviderAdmission<'_>,
    issuers: Result<Vec<&language_semantics::BoundaryQualificationIssuer>, &'static str>,
    candidates: &[&ProviderPlan],
    facts: effects::SelectedProviderPlanFacts,
    declarations: Result<Vec<Result<SelectedPlanDeclarations<'_>, Diagnostic>>, Diagnostic>,
    root_grants: &[String],
    authored_root_grants: &[AuthoredRootGrant],
) -> Result<SelectedProviderPlanBinding, Vec<Diagnostic>> {
    let provider_grants =
        effects::provider_grants::resolve_selected_provider_grants(candidates, &facts, root_grants)
            .map_err(|diagnostic| vec![diagnostic])?;
    let grants = effects::provider_grants::resolve_authored_selected_provider_grants(
        candidates,
        &facts,
        authored_root_grants,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    crate::validate_qualification_grants(
        issuers.map_err(|reason| vec![Diagnostic::error(reason)])?,
        &facts,
        &provider_grants,
    )?;
    boundary_admission.admit(candidates, &facts)?;
    let declarations = declarations.map_err(|error| vec![error])?;
    let selected = enrich_selected_plans(facts, &declarations)?;
    Ok(SelectedProviderPlanBinding { selected, grants })
}

fn enrich_selected_plans(
    facts: effects::SelectedProviderPlanFacts,
    declarations: &[Result<SelectedPlanDeclarations<'_>, Diagnostic>],
) -> Result<effects::SelectedProviderPlanFacts, Vec<Diagnostic>> {
    if facts.plans().len() != declarations.len() {
        return Err(vec![Diagnostic::error(
            "installation reach lost selected provider custody",
        )]);
    }
    let mut reaches = Vec::new();
    let mut diagnostics = Vec::new();
    for (plan, prepared) in facts.plans().iter().zip(declarations) {
        let source = match prepared {
            Ok(prepared)
                if plan == prepared.plan && prepared.declarations.rows.len() == plan.rows.len() =>
            {
                &prepared.declarations
            }
            Ok(_) => {
                diagnostics.push(Diagnostic::error(
                    "installation reach lost exact selected row custody",
                ));
                continue;
            }
            Err(error) => {
                diagnostics.push(error.clone());
                continue;
            }
        };
        for row in &source.rows {
            match &row.reach {
                Ok(Some(reach)) => reaches.push(crate::InstallationReachRow {
                    requirement_identity: reach.requirement_identity.clone(),
                    provider_plan_report_identity: plan.report_fingerprint(),
                    provider_name: plan.provider_type.clone(),
                    realization_name: reach.realization_name.clone(),
                    upper_bound: reach.upper_bound.clone(),
                    concrete_reach: reach.concrete_reach.clone(),
                    dependencies: reach.dependencies.clone(),
                }),
                Ok(None) => {}
                Err(reason) => diagnostics.push(Diagnostic::error(reason.clone())),
            }
        }
    }
    let reach = crate::resolve_installation_reach(reaches);
    let (reach, reach_identities) = match reach {
        Ok(reach) if diagnostics.is_empty() => reach,
        Ok(_) => return Err(diagnostics),
        Err(errors) => {
            diagnostics.extend(errors);
            return Err(diagnostics);
        }
    };

    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let mut aliases = Vec::new();
    for prepared in declarations {
        // The preceding phase admitted every declaration view and row alignment.
        let prepared = prepared.as_ref().map_err(|error| vec![error.clone()])?;
        let source = &prepared.declarations;
        for (row, declaration) in prepared.plan.rows.iter().zip(&source.rows) {
            match &declaration.leaf_applications {
                Ok(applications) => {
                    if applications.is_empty() {
                        continue;
                    }
                    let template =
                        identities.intern(prepared.plan.identities.text(row.requirement_identity));
                    for application in applications {
                        aliases.push(effects::SelectedRequirementSpecialization {
                            provider_plan_report_identity: prepared.plan.report_fingerprint(),
                            selected_requirement_identity: template,
                            selected_machine_identity: template,
                            requirement_identity: identities.intern(application),
                            machine_identity: template,
                        });
                    }
                }
                Err(errors) => diagnostics.extend(errors.iter().cloned().map(Diagnostic::error)),
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let alias_identities = identities.build();

    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let mut applications = Vec::new();
    for prepared in declarations {
        let prepared = prepared.as_ref().map_err(|error| vec![error.clone()])?;
        let source = &prepared.declarations;
        let plan = prepared.plan;
        if !plan.rows.iter().zip(&source.rows).any(|(row, source)| {
            matches!(row.binding, ProviderBinding::Import { .. })
                && source.external_applications.has_closed_application
        }) {
            continue;
        }
        if declarations
            .iter()
            .filter(|other| other.as_ref().is_ok_and(|other| other.plan == plan))
            .count()
            != 1
        {
            return Err(vec![Diagnostic::error(
                "selected Import plan lost its unique exact declaration provenance",
            )]);
        }
        for (row, declaration) in plan.rows.iter().zip(&source.rows) {
            if !matches!(row.binding, ProviderBinding::Import { .. }) {
                continue;
            }
            for application in declaration
                .external_applications
                .applications
                .as_ref()
                .map_err(|reason| vec![Diagnostic::error(reason.clone())])?
            {
                applications.push(effects::SelectedExternalRequirementApplication {
                    provider_plan_report_identity: plan.report_fingerprint(),
                    provider_plan_digest: plan.identity_digest(),
                    selected_requirement_identity: identities
                        .intern(plan.identities.text(row.requirement_identity)),
                    requirement_identity: identities.intern(application),
                });
            }
        }
    }
    facts
        .with_specialized_requirement_rows(aliases, &alias_identities)
        .and_then(|facts| {
            facts.with_external_requirement_applications(applications, &identities.build())
        })
        .and_then(|facts| facts.with_installation_reach_resolutions(reach, &reach_identities))
        .map_err(|reason| vec![Diagnostic::error(reason)])
}
