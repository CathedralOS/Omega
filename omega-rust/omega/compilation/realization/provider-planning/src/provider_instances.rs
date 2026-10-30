//! Prepare source declarations for exact selected provider instances.
//!
//! Source lookup stays here and in Psi's checked declaration producer. The
//! source-free provider-selection owner judges counts, coverage and attachment.
//! Callback preparation shares these same records and exact adapter judgments.

use crate::exact_checked_adapter;
use checked_trees::typed_trees::machine::Machine;
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};

struct CheckedAdapterDeclaration<'typed, 'plan> {
    plan: &'plan ProviderPlan,
    row: &'plan ProviderPlanRow,
    machine: Result<&'typed Machine, Diagnostic>,
}

/// Invocation scratch: one immutable source judgment per original selected row.
/// Errors remain data until each existing expansion site consumes that row.
struct CheckedAdapterDeclarations<'typed, 'plan> {
    rows: Vec<CheckedAdapterDeclaration<'typed, 'plan>>,
}

impl<'typed, 'plan> CheckedAdapterDeclarations<'typed, 'plan> {
    fn prepare(
        typed: &'typed checked_trees::typed_trees::TypedTrees,
        selected: &'plan effects::SelectedProviderPlanFacts,
    ) -> Self {
        let context = crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product);
        let mut rows = Vec::new();
        for plan in selected.plans() {
            for row in &plan.rows {
                if matches!(row.binding, ProviderBinding::CheckedAdapter { .. }) {
                    rows.push(CheckedAdapterDeclaration {
                        plan,
                        row,
                        machine: exact_checked_adapter(context, plan, row),
                    });
                }
            }
        }
        Self { rows }
    }

    fn machine(
        &self,
        plan: &ProviderPlan,
        row: &ProviderPlanRow,
    ) -> Result<&'typed Machine, Diagnostic> {
        self.rows
            .iter()
            .find(|prepared| std::ptr::eq(prepared.plan, plan) && std::ptr::eq(prepared.row, row))
            .ok_or_else(|| {
                Diagnostic::error(
                    "checked provider instance lost its original prepared plan/row custody",
                )
            })?
            .machine
            .clone()
    }

    fn callback_declarations(&self) -> Vec<provider_selection::SelectedCallbackRow<'plan>> {
        self.rows.iter().map(|prepared| provider_selection::SelectedCallbackRow {
            plan: prepared.plan,
            row: prepared.row,
            checked_adapter_body: prepared.machine.as_ref()
                .map(|machine| machine.supplies_checked_body())
                .map_err(|diagnostic| format!("callback forwarding provider is not its exact checked adapter: {diagnostic:?}")),
        }).collect()
    }
}

/// Complete source-only instance preparation, then return only whole plans,
/// exact row facts and closed applications. The result borrows selected-plan
/// custody, not the checked program. Callback policy runs in provider-selection.
pub fn prepare_callback_providers<'plan>(
    program: &checked_trees::CheckedTrees,
    selected: &'plan effects::SelectedProviderPlanFacts,
    declarations: &terminal_psi::CallbackDeclarationRoster,
) -> Result<provider_selection::CallbackProviderDeclarations<'plan>, Vec<Diagnostic>> {
    if declarations.uses.is_empty() {
        return Ok(provider_selection::CallbackProviderDeclarations::unused());
    }
    let adapters = CheckedAdapterDeclarations::prepare(&program.typed, selected);
    let instances = project_with_adapters(&program.typed, &program.facts, selected, &adapters)?;
    Ok(provider_selection::CallbackProviderDeclarations::prepared(
        instances,
        adapters.callback_declarations(),
    ))
}

/// Source-only consumers request this projection at their boundary. Ordinary
/// native production must not reconstruct checked instances just to discard
/// them when it joins the canonical Terminal provider roster.
pub fn project_checked_provider_instances(
    typed: &checked_trees::typed_trees::TypedTrees,
    check_facts: &checked_trees::CheckFacts,
    selected: &effects::SelectedProviderPlanFacts,
) -> Result<effects::SelectedProviderPlanFacts, Vec<diagnostics::Diagnostic>> {
    let adapters = CheckedAdapterDeclarations::prepare(typed, selected);
    project_with_adapters(typed, check_facts, selected, &adapters)
}

fn project_with_adapters(
    typed: &checked_trees::typed_trees::TypedTrees,
    check_facts: &checked_trees::CheckFacts,
    selected: &effects::SelectedProviderPlanFacts,
    adapters: &CheckedAdapterDeclarations<'_, '_>,
) -> Result<effects::SelectedProviderPlanFacts, Vec<Diagnostic>> {
    let declarations = prepare_instance_declarations(typed, check_facts, selected, adapters);
    provider_selection::project_provider_instances(selected, &declarations)
}

fn prepare_instance_declarations<'plan>(
    typed: &checked_trees::typed_trees::TypedTrees,
    check_facts: &checked_trees::CheckFacts,
    selected: &'plan effects::SelectedProviderPlanFacts,
    adapters: &CheckedAdapterDeclarations<'_, '_>,
) -> Vec<provider_selection::ProviderInstanceDeclarations<'plan>> {
    use language_semantics::provider_declarations::{
        ClosedProviderInstance, ProviderInstanceApplication,
    };
    use provider_selection::{
        GenericProviderInstanceDeclarations, ProviderInstanceDeclarations,
        ProviderInstanceRowDeclarations,
    };
    let context = crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product);
    let mut templates = Vec::new();
    for requirement in typed.machines().iter() {
        if requirement.supply_mode != language_semantics::MachineSupplyMode::TopLevelRequirement
            || typed.machine_type_parameters(requirement).is_empty()
        {
            continue;
        }
        if let Some(schema) =
            crate::service_schema::from_typed_boundary_requirement(context, requirement)
        {
            templates.push((requirement, schema));
        }
    }
    let mut declarations = Vec::new();
    for plan in selected.plans() {
        let selecting =
            crate::provider_identity::named_provider(context, plan).map(|owner| owner.symbol);
        let mut rows = Vec::new();
        for row in &plan.rows {
            let inherited = match row.binding {
                ProviderBinding::CompilerIntrinsic { .. } | ProviderBinding::Syscall { .. } => {
                    let template = plan.identities.text(row.requirement_identity);
                    checked_trees::provider_declarations::closed_provider_leaf_applications(
                        typed, template,
                    )
                    .map(|applications| {
                        applications
                            .into_iter()
                            .map(|requirement| ProviderInstanceApplication {
                                candidates: vec![Ok(ClosedProviderInstance {
                                    requirement,
                                    realization: template.to_owned(),
                                })],
                            })
                            .collect()
                    })
                    .map_err(|reasons| reasons.into_iter().map(Diagnostic::error).collect())
                }
                ProviderBinding::CheckedAdapter {
                    machine_package_identity,
                    ..
                } => adapters
                    .machine(plan, row)
                    .map(|provider| {
                        let applications = checked_trees::provider_instances::inherited_instances(
                            typed,
                            check_facts,
                            plan.identities.text(row.requirement_identity),
                            provider,
                            selecting,
                            machine_package_identity,
                        );
                        let mut projected = Vec::new();
                        for application in applications {
                            let candidates = application
                                .candidates
                                .into_iter()
                                .map(|candidate| {
                                    candidate.map(|instance| render_instance(typed, instance))
                                })
                                .collect();
                            projected.push(ProviderInstanceApplication { candidates });
                        }
                        projected
                    })
                    .map_err(|error| vec![error]),
                _ => Ok(Vec::new()),
            };
            rows.push(ProviderInstanceRowDeclarations { inherited });
        }
        let mut generic_templates = Vec::new();
        for (template, schema) in &templates {
            if *schema != plan.schema {
                continue;
            }
            let mut instance_rows = Vec::new();
            for row in &plan.rows {
                let instances = match row.binding {
                    ProviderBinding::CheckedAdapter {
                        machine_package_identity,
                        ..
                    } => adapters.machine(plan, row).map(|provider| {
                        checked_trees::provider_instances::generic_instances(
                            typed,
                            check_facts,
                            provider,
                            template,
                            machine_package_identity,
                        )
                        .into_iter()
                        .map(|instance| instance.map(|instance| render_instance(typed, instance)))
                        .collect()
                    }),
                    _ => Ok(Vec::new()),
                };
                instance_rows.push(instances);
            }
            generic_templates.push(GenericProviderInstanceDeclarations {
                template: typed
                    .machine_callable_identity_text(template)
                    .unwrap_or_default(),
                rows: instance_rows,
                required_instances: checked_trees::provider_instances::closed_requirements(
                    typed, template,
                )
                .into_iter()
                .map(|identity| typed.callable_identity_text(identity))
                .collect(),
            });
        }
        declarations.push(ProviderInstanceDeclarations {
            plan,
            rows,
            generic_templates,
        });
    }
    declarations
}

/// Rendering happens once at the existing plan/review encoding boundary.
/// The checked declaration producer retains handles in its own identity owner.
fn render_instance(
    typed: &checked_trees::typed_trees::TypedTrees,
    instance: language_semantics::provider_declarations::ClosedProviderInstance<
        checked_trees::typed_trees::type_identity::CallableIdentityId,
    >,
) -> language_semantics::provider_declarations::ClosedProviderInstance<String> {
    language_semantics::provider_declarations::ClosedProviderInstance {
        requirement: typed.callable_identity_text(instance.requirement),
        realization: typed.callable_identity_text(instance.realization),
    }
}
