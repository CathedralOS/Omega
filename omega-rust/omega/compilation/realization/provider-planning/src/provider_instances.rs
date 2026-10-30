//! Retain selected source correspondence once, then lend declaration-only views.
//!
//! The immutable checked owner and complete selected plan roster stay coupled.
//! Callbacks, checking, review and reference execution share lazy preparation,
//! but each consumes source-free policy at its existing admission point. Cache
//! population is not a provider grant or part of semantic equality.

use crate::exact_checked_adapter;
use checked_trees::typed_trees::machine::Machine;
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};
use std::sync::{Arc, OnceLock};

pub struct SelectedProviderSource {
    program: Arc<checked_trees::CheckedTrees>,
    plans: effects::SelectedProviderPlanFacts,
    declarations: OnceLock<PreparedProviderInstances>,
}

struct PreparedProviderInstances {
    plans: Vec<provider_selection::ProviderPlanInstanceDeclarations>,
    checked_bodies: Vec<Vec<Result<bool, Diagnostic>>>,
}

impl std::fmt::Debug for SelectedProviderSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SelectedProviderSource")
            .field("plans", &self.plans)
            .finish_non_exhaustive()
    }
}

impl PartialEq for SelectedProviderSource {
    fn eq(&self, other: &Self) -> bool {
        // As with the external-declaration product, equal-looking source from
        // another checked owner is not interchangeable. Derived cache state is.
        Arc::ptr_eq(&self.program, &other.program) && self.plans == other.plans
    }
}
impl Eq for SelectedProviderSource {}

impl SelectedProviderSource {
    pub fn new(
        program: Arc<checked_trees::CheckedTrees>,
        plans: effects::SelectedProviderPlanFacts,
    ) -> Self {
        Self {
            program,
            plans,
            declarations: OnceLock::new(),
        }
    }

    pub fn program(&self) -> &Arc<checked_trees::CheckedTrees> {
        &self.program
    }
    pub fn plans(&self) -> &effects::SelectedProviderPlanFacts {
        &self.plans
    }

    /// Admission remains consumer-owned. Preparing declarations neither reports
    /// their deferred failures nor admits this projected provider closure.
    pub fn project_instances(&self) -> Result<effects::SelectedProviderPlanFacts, Vec<Diagnostic>> {
        let prepared = self.declarations();
        let declarations = self
            .plans
            .plans()
            .iter()
            .zip(&prepared.plans)
            .map(
                |(plan, instances)| provider_selection::ProviderInstanceDeclarations {
                    plan,
                    instances,
                },
            )
            .collect::<Vec<_>>();
        provider_selection::project_provider_instances(&self.plans, &declarations)
    }

    /// A native request with no nominal uses does no instance preparation.
    pub fn callback_providers(
        &self,
        declarations: &terminal_psi::CallbackDeclarationRoster,
    ) -> Result<provider_selection::CallbackProviderDeclarations<'_>, Vec<Diagnostic>> {
        if declarations.uses.is_empty() {
            return Ok(provider_selection::CallbackProviderDeclarations::unused());
        }
        let instances = self.project_instances()?;
        let prepared = self.declarations();
        let mut checked_adapters = Vec::new();
        for (plan, bodies) in self.plans.plans().iter().zip(&prepared.checked_bodies) {
            for (row, body) in plan.rows.iter().zip(bodies) {
                if !matches!(row.binding, ProviderBinding::CheckedAdapter { .. }) {
                    continue;
                }
                checked_adapters.push(provider_selection::SelectedCallbackRow {
                    plan,
                    row,
                    checked_adapter_body: body.clone().map_err(|diagnostic| format!(
                        "callback forwarding provider is not its exact checked adapter: {diagnostic:?}"
                    )),
                });
            }
        }
        Ok(provider_selection::CallbackProviderDeclarations::prepared(
            instances,
            checked_adapters,
        ))
    }

    fn declarations(&self) -> &PreparedProviderInstances {
        self.declarations.get_or_init(|| {
            let adapters = CheckedAdapterDeclarations::prepare(&self.program.typed, &self.plans);
            let plans = prepare_instance_declarations(
                &self.program.typed,
                &self.program.facts,
                &self.plans,
                &adapters,
            );
            let checked_bodies = self
                .plans
                .plans()
                .iter()
                .map(|plan| {
                    plan.rows
                        .iter()
                        .map(|row| {
                            if matches!(row.binding, ProviderBinding::CheckedAdapter { .. }) {
                                adapters
                                    .machine(plan, row)
                                    .map(Machine::supplies_checked_body)
                            } else {
                                Ok(false)
                            }
                        })
                        .collect()
                })
                .collect();
            PreparedProviderInstances {
                plans,
                checked_bodies,
            }
        })
    }
}

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
}

fn prepare_instance_declarations(
    typed: &checked_trees::typed_trees::TypedTrees,
    check_facts: &checked_trees::CheckFacts,
    selected: &effects::SelectedProviderPlanFacts,
    adapters: &CheckedAdapterDeclarations<'_, '_>,
) -> Vec<provider_selection::ProviderPlanInstanceDeclarations> {
    use language_semantics::provider_declarations::{
        ClosedProviderInstance, ProviderInstanceApplication,
    };
    use provider_selection::{
        GenericProviderInstanceDeclarations, ProviderInstanceRowDeclarations,
        ProviderPlanInstanceDeclarations,
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
        declarations.push(ProviderPlanInstanceDeclarations {
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
