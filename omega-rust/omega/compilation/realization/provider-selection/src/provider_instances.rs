//! Attach exact checked instances to whole selected plans, without source.
//!
//! Psi supplies declaration groups; this owner decides selected cardinality,
//! template-row agreement and complete closed-requirement coverage. Failures
//! stay deferred until the same row/application asks for them.

use diagnostics::Diagnostic;
use effects::SelectedProviderPlanFacts;
use effects::provider_plan::{ProviderBinding, ProviderIdentityId, ProviderPlan, ProviderPlanRow};
use language_semantics::provider_declarations::{
    ClosedProviderInstance, ProviderInstanceApplication,
};

pub struct ProviderInstanceRowDeclarations {
    pub inherited: Result<Vec<ProviderInstanceApplication>, Vec<Diagnostic>>,
}

pub struct GenericProviderInstanceDeclarations {
    pub template: String,
    pub rows: Vec<Result<Vec<Result<ClosedProviderInstance, String>>, Diagnostic>>,
    pub required_instances: Vec<String>,
}

/// Invocation-local correspondence borrows the original complete selected plan.
/// No checked program, source symbol or machine reference reaches this owner.
pub struct ProviderInstanceDeclarations<'plan> {
    pub plan: &'plan ProviderPlan,
    pub rows: Vec<ProviderInstanceRowDeclarations>,
    pub generic_templates: Vec<GenericProviderInstanceDeclarations>,
}

pub fn project_provider_instances(
    selected: &SelectedProviderPlanFacts,
    declarations: &[ProviderInstanceDeclarations<'_>],
) -> Result<SelectedProviderPlanFacts, Vec<Diagnostic>> {
    if declarations.len() != selected.plans().len() {
        return Err(vec![Diagnostic::error(
            "provider instance declarations lost their complete selected plan roster",
        )]);
    }
    let mut diagnostics = Vec::new();
    let mut rows = Vec::new();
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    for (plan, declaration) in selected.plans().iter().zip(declarations) {
        if !std::ptr::eq(plan, declaration.plan) || declaration.rows.len() != plan.rows.len() {
            return Err(vec![Diagnostic::error(
                "provider instance declarations lost their original plan/row custody",
            )]);
        }
        for (row, declaration) in plan.rows.iter().zip(&declaration.rows) {
            let applications = match &declaration.inherited {
                Ok(applications) => applications,
                Err(errors) => {
                    diagnostics.extend(errors.iter().cloned());
                    continue;
                }
            };
            let selected_machine = match row.binding {
                ProviderBinding::CheckedAdapter {
                    machine_identity, ..
                } => machine_identity,
                ProviderBinding::CompilerIntrinsic { .. } | ProviderBinding::Syscall { .. } => {
                    row.requirement_identity
                }
                _ => continue,
            };
            for application in applications {
                let instance = match application.candidates.as_slice() {
                    [] => continue,
                    [instance] => instance,
                    candidates => {
                        diagnostics.push(Diagnostic::error(format!(
                            "selected provider plan `{}` resolves requirement family coordinate to {} exact checked realization instances; exactly one must serve it",
                            plan.name, candidates.len(),
                        )));
                        continue;
                    }
                };
                match instance {
                    Ok(instance) => append_instance(
                        plan,
                        row,
                        selected_machine,
                        instance,
                        &mut rows,
                        &mut identities,
                    ),
                    Err(reason) => diagnostics.push(Diagnostic::error(reason.clone())),
                }
            }
        }
        let [template] = declaration.generic_templates.as_slice() else {
            if declaration.generic_templates.len() > 1 {
                diagnostics.push(Diagnostic::error(format!(
                    "selected provider plan `{}` binds {} generic requirement templates",
                    plan.name,
                    declaration.generic_templates.len(),
                )));
            }
            continue;
        };
        if template.rows.len() != plan.rows.len() {
            return Err(vec![Diagnostic::error(
                "generic provider instance declarations lost their selected rows",
            )]);
        }
        for (row, instances) in plan.rows.iter().zip(&template.rows) {
            let ProviderBinding::CheckedAdapter {
                machine_identity, ..
            } = row.binding
            else {
                continue;
            };
            if plan.identities.text(row.requirement_identity) != template.template {
                diagnostics.push(Diagnostic::error(format!(
                    "selected provider plan `{}` checked-adapter row `{}` does not retain the generic requirement template `{}` its schema binds",
                    plan.name, plan.identities.text(row.requirement_identity), template.template,
                )));
                continue;
            }
            let instances = match instances {
                Ok(instances) => instances,
                Err(error) => {
                    diagnostics.push(error.clone());
                    continue;
                }
            };
            for instance in instances {
                match instance {
                    Ok(instance) => append_instance(
                        plan,
                        row,
                        machine_identity,
                        instance,
                        &mut rows,
                        &mut identities,
                    ),
                    Err(reason) => diagnostics.push(Diagnostic::error(reason.clone())),
                }
            }
        }
        for requirement in &template.required_instances {
            let matching = rows
                .iter()
                .filter(|row| {
                    row.provider_plan_report_identity == plan.report_fingerprint()
                        && identities.text(row.requirement_identity) == Some(requirement.as_str())
                })
                .count();
            if matching != 1 {
                diagnostics.push(Diagnostic::error(format!(
                    "selected provider plan `{}` resolves retained closed requirement `{requirement}` to {matching} specialized provider instances; exactly one must serve it", plan.name,
                )));
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    selected
        .clone()
        .with_specialized_requirement_rows(rows, &identities.build())
        .map_err(|reason| vec![Diagnostic::error(reason)])
}

fn append_instance(
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    selected_machine: ProviderIdentityId,
    instance: &ClosedProviderInstance,
    rows: &mut Vec<effects::SelectedRequirementSpecialization>,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
) {
    rows.push(effects::SelectedRequirementSpecialization {
        provider_plan_report_identity: plan.report_fingerprint(),
        selected_requirement_identity: identities
            .intern(plan.identities.text(row.requirement_identity)),
        selected_machine_identity: identities.intern(plan.identity_text(selected_machine)),
        requirement_identity: identities.intern(&instance.requirement),
        machine_identity: identities.intern(&instance.realization),
    });
}
