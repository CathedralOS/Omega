/*
Selected provider plan binding, qualification-grant validation, installation
reach closure, and checked provider instance attachment.

Psi has already projected exact declaration associations. We own selected-plan
policy and phase order without any source-tree input:
- bind_selected_provider_plan_facts resolves provider grants and qualification
  grants before boundary policy, then enriches selected plans with installation
  reach, physical leaf aliases, and external Import applications while
  preserving their independent diagnostic phases.
- validate_qualification_grants checks that qualification requirements have
  unambiguous selected grants without stamping a target's report identity into
  the checked program; a shared inherited requirement carried by several granted
  plans still has one grant owner (the plan whose schema is the requirement's
  own boundary trait), and when that owner slot is not granted the unscoped
  matches keep their fail-closed behavior rather than attributing authority to
  an inheriting plan by roster order.
- installation_reach and provider_instances complete the installation closure
  and attach exact checked instances to whole selected plans.
*/

mod installation_reach;
mod provider_instances;

pub use installation_reach::{InstallationReachRow, resolve_installation_reach};
pub use provider_instances::{
    GenericProviderInstanceDeclarations, ProviderInstanceDeclarations,
    ProviderInstanceRowDeclarations, ProviderPlanInstanceDeclarations, project_provider_instances,
};

use diagnostics::Diagnostic;
use effects::provider_grants::{AuthoredRootGrant, ResolvedAuthoredSelectedProviderGrant};
use effects::provider_plan::{ProviderBinding, ProviderPlan};
use language_semantics::provider_declarations::ProviderDeclarations;

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
    validate_qualification_grants(
        issuers.map_err(|reason| vec![Diagnostic::error(reason)])?,
        &facts,
        &provider_grants,
    )?;
    boundary_admission.admit(&facts)?;
    let declarations = declarations.map_err(|error| vec![error])?;
    let selected = enrich_selected_plans(facts, &declarations)?;
    Ok(SelectedProviderPlanBinding { selected, grants })
}

pub fn validate_qualification_grants<'issuer>(
    issuers: impl IntoIterator<Item = &'issuer language_semantics::BoundaryQualificationIssuer>,
    facts: &effects::SelectedProviderPlanFacts,
    provider_grants: &[effects::provider_grants::ResolvedSelectedProviderGrant],
) -> Result<(), Vec<Diagnostic>> {
    let mut granted_plans = Vec::new();
    for grant in provider_grants {
        let exact_selected_matches = facts
            .plans()
            .iter()
            .filter(|plan| grant.replays_selected_plan(plan))
            .count();
        if exact_selected_matches != 1 {
            return Err(vec![Diagnostic::error(format!(
                "provider grant `{}` replays against {exact_selected_matches} exact selected provider plans",
                grant.selector,
            ))]);
        }
        if !granted_plans.iter().any(
            |retained: &&effects::provider_grants::ResolvedSelectedProviderGrant| {
                retained.selected_plan == grant.selected_plan
                    && retained.selected_plan_digest == grant.selected_plan_digest
            },
        ) {
            granted_plans.push(grant);
        }
    }
    let mut receipt_diagnostics = Vec::new();
    for issuer in issuers {
        let requirement_identity = &issuer.requirement_identity;
        let matches = granted_plans
            .iter()
            .filter(|grant| {
                grant
                    .selected_plan
                    .schema
                    .declaration
                    .methods
                    .iter()
                    .any(|method| {
                        grant
                            .selected_plan
                            .schema
                            .identity_text(method.requirement_identity)
                            == requirement_identity.as_str()
                    })
            })
            .collect::<Vec<_>>();
        let owner_matches = matches
            .iter()
            .copied()
            .filter(|grant| {
                issuer.is_product_owner
                    && grant
                        .selected_plan
                        .schema
                        .identity_text(grant.selected_plan.schema.declaration.trait_name)
                        == issuer.boundary_path
                    && grant
                        .selected_plan
                        .schema
                        .declaration
                        .trait_package_identity
                        == issuer.boundary_package
            })
            .collect::<Vec<_>>();
        let matches = if owner_matches.is_empty() {
            matches
        } else {
            owner_matches
        };
        match matches.as_slice() {
            [] | [_] => {}
            _ => receipt_diagnostics.push(Diagnostic::error(format!(
                "admitted qualification requirement `{requirement_identity}` matches {} granted selected provider plans",
                matches.len()
            ))),
        }
    }
    if !receipt_diagnostics.is_empty() {
        return Err(receipt_diagnostics);
    }
    Ok(())
}

pub(crate) fn enrich_selected_plans(
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
                Ok(Some(reach)) => reaches.push(InstallationReachRow {
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
    let reach = resolve_installation_reach(reaches);
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
                        if !prepared
                            .plan
                            .schema
                            .declaration
                            .matches_receiving_application(
                                prepared.plan.identity_text(row.requirement_identity),
                                Some(&application.receiving_application),
                            )
                        {
                            continue;
                        }
                        aliases.push(effects::SelectedRequirementSpecialization {
                            provider_plan_report_identity: prepared.plan.report_fingerprint(),
                            selected_requirement_identity: template,
                            selected_machine_identity: template,
                            requirement_identity: identities.intern(&application.requirement),
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
