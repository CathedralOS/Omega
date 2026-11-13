// Selected provider source custody and instance projection for explicit
// inspection. Equal-looking source from
// another checked owner is not interchangeable (`Arc::ptr_eq`), whereas derived
// instance declaration cache state is. Preparing declarations neither reports
// deferred failures nor admits the projected provider closure.

use crate::exact_checked_adapter;
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};
use std::sync::{Arc, OnceLock};
use typed_trees::machine::Machine;

pub struct SelectedProviderSource {
    inventory: Arc<crate::PreparedProviderInventory>,
    plans: Arc<effects::SelectedProviderPlanFacts>,
    declarations: OnceLock<PreparedProviderInstances>,
}

struct PreparedProviderInstances {
    plans: Result<Vec<provider_selection::ProviderPlanInstanceDeclarations>, Vec<Diagnostic>>,
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
        Arc::ptr_eq(self.program(), other.program()) && self.plans == other.plans
    }
}
impl Eq for SelectedProviderSource {}

impl SelectedProviderSource {
    pub fn new(
        inventory: Arc<crate::PreparedProviderInventory>,
        plans: Arc<effects::SelectedProviderPlanFacts>,
    ) -> Self {
        Self {
            inventory,
            plans,
            declarations: OnceLock::new(),
        }
    }

    pub fn program(&self) -> &Arc<crate::PublishedProviderSource> {
        self.inventory.program()
    }
    pub fn inventory(&self) -> &Arc<crate::PreparedProviderInventory> {
        &self.inventory
    }
    pub fn plans(&self) -> &effects::SelectedProviderPlanFacts {
        &self.plans
    }

    pub fn project_instances(&self) -> Result<effects::SelectedProviderPlanFacts, Vec<Diagnostic>> {
        let prepared = self.declarations();
        let instances = prepared.plans.as_ref().map_err(Clone::clone)?;
        let declarations = self
            .plans
            .plans()
            .iter()
            .zip(instances)
            .map(
                |(plan, instances)| provider_selection::ProviderInstanceDeclarations {
                    plan,
                    instances,
                },
            )
            .collect::<Vec<_>>();
        provider_selection::project_provider_instances(&self.plans, &declarations)
    }

    fn declarations(&self) -> &PreparedProviderInstances {
        self.declarations.get_or_init(|| {
            let adapters = CheckedAdapterDeclarations::prepare(&self.program().typed, &self.plans);
            let plans = prepare_instance_declarations(
                &self.program().typed,
                self.program(),
                &self.plans,
                &adapters,
            );
            PreparedProviderInstances { plans }
        })
    }
}

struct CheckedAdapterDeclaration<'typed, 'plan> {
    plan: &'plan ProviderPlan,
    row: &'plan ProviderPlanRow,
    machine: Result<&'typed Machine, Diagnostic>,
}

struct CheckedAdapterDeclarations<'typed, 'plan> {
    rows: Vec<CheckedAdapterDeclaration<'typed, 'plan>>,
}

impl<'typed, 'plan> CheckedAdapterDeclarations<'typed, 'plan> {
    fn prepare(
        typed: &'typed typed_trees::TypedTrees,
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
    typed: &typed_trees::TypedTrees,
    source: &crate::PublishedProviderSource,
    selected: &effects::SelectedProviderPlanFacts,
    adapters: &CheckedAdapterDeclarations<'_, '_>,
) -> Result<Vec<provider_selection::ProviderPlanInstanceDeclarations>, Vec<Diagnostic>> {
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
                    typed_trees::provider_declarations::closed_provider_leaf_applications(
                        typed, template,
                    )
                    .map(|applications| {
                        applications
                            .into_iter()
                            .map(|application| ProviderInstanceApplication {
                                declaration_application: Ok(application
                                    .receiving_application
                                    .clone()),
                                receiving_application: Ok(application.receiving_application),
                                candidates: vec![Ok(ClosedProviderInstance {
                                    requirement: application.requirement,
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
                        inherited_instances(
                            source,
                            plan.identities.text(row.requirement_identity),
                            provider,
                            selecting,
                            machine_package_identity,
                        )
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
                        generic_instances(source, provider, template, machine_package_identity)
                    }),
                    _ => Ok(Vec::new()),
                };
                instance_rows.push(instances);
            }
            generic_templates.push(GenericProviderInstanceDeclarations {
                template: typed
                    .machine_callable_identity_text(template)
                    .filter(|identity| !identity.is_empty())
                    .ok_or_else(|| {
                        vec![Diagnostic::error(format!(
                            "generic provider template `{}` has no complete callable identity",
                            typed.symbols.display_path(template.symbol, "::"),
                        ))]
                    })?,
                rows: instance_rows,
                required_instances:
                    typed_trees::provider_declarations::closed_provider_requirements(
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
    Ok(declarations)
}

fn inherited_instances(
    source: &crate::PublishedProviderSource,
    requirement: &str,
    provider: &Machine,
    selecting: Option<symbols::SymbolHandle>,
    package: Option<semantic_vocabulary::PackageKeyIdentity>,
) -> Vec<language_semantics::provider_declarations::ProviderInstanceApplication<String>> {
    use language_semantics::provider_declarations::{
        ClosedProviderInstance, ProviderInstanceApplication,
    };
    let typed = &source.typed;
    let module = source.module();
    let provider_identity = typed.machine_callable_identity_text(provider);
    let selecting_identity = selecting.map(|symbol| {
        let sources =
            source::source_identity::toolchain_source_identities(typed.symbols.source_files())?;
        typed
            .exact_owner_nominal_identity(symbol, &sources)
            .and_then(|identity| typed.type_identity_text(identity))
            .ok_or_else(|| "provider instance lost its exact selecting declaration".to_owned())
    });
    typed
        .closed_requirement_applications()
        .filter(|application| {
            typed
                .trait_machine_signature(application.template)
                .is_some_and(|(owner, signature)| {
                    typed.trait_requirement_callable_identity_text(owner, signature) == requirement
                })
        })
        .map(|application| {
            let expected = typed
                .trait_definition_by_symbol(application.declaring_trait)
                .map(|owner| {
                    typed.trait_requirement_callable_identity_text(owner, application.signature)
                });
            let receiving = typed
                .type_identity_text(application.portable_trait_application_identity)
                .ok_or_else(|| {
                    "provider instance lost its portable receiving application".to_owned()
                });
            let mut candidates = Vec::new();
            let mut declaration_application: Option<Result<String, String>> = None;
            for row in &module.provider_candidates {
                if module.identities.text(row.source_requirement_identity) != Some(requirement)
                    || module.identities.text(row.source_candidate_identity)
                        != provider_identity.as_deref()
                    || row.source_candidate_package_identity != package
                    || module.identities.text(row.requirement_identity) != expected.as_deref()
                    || module.identities.text(row.receiving_application)
                        != receiving.as_ref().ok().map(String::as_str)
                {
                    continue;
                }
                if let Some(Ok(expected)) = &selecting_identity
                    && module.identities.text(row.provider_declaration.identity())
                        != Some(expected.as_str())
                {
                    continue;
                }
                let declared = module
                    .identities
                    .text(row.declaration_application)
                    .filter(|identity| !identity.is_empty())
                    .map(str::to_owned)
                    .ok_or_else(|| "provider instance lost its declaration application".to_owned());
                declaration_application = Some(match declaration_application.take() {
                    Some(previous) if previous != declared => {
                        Err("provider group has competing declaration applications".into())
                    }
                    Some(previous) => previous,
                    None => declared,
                });
                let candidate = (|| {
                    if let Some(expected) = &selecting_identity {
                        expected.as_ref().map_err(Clone::clone)?;
                    }
                    let required = module
                        .identities
                        .text(row.requirement_identity)
                        .ok_or("provider instance lost its required identity")?;
                    let actual = module
                        .identities
                        .text(row.candidate_identity)
                        .ok_or("provider instance lost its candidate identity")?;
                    if module
                        .machine(row.candidate)
                        .and_then(|machine| machine.declaration.as_ref())
                        .and_then(|declaration| module.identities.text(declaration.callable))
                        != Some(actual)
                    {
                        return Err(
                            "provider instance differs from its accepted candidate".to_owned()
                        );
                    }
                    Ok(ClosedProviderInstance {
                        requirement: required.to_owned(),
                        realization: actual.to_owned(),
                    })
                })();
                candidates.push(candidate);
            }
            if candidates.is_empty() {
                candidates.push(Err(
                    "provider instance has no accepted application refinement receipt".into(),
                ));
            }
            ProviderInstanceApplication {
                declaration_application: declaration_application.unwrap_or_else(|| {
                    Err("provider group has no accepted declaration application".into())
                }),
                receiving_application: receiving,
                candidates,
            }
        })
        .collect()
}

fn generic_instances(
    source: &crate::PublishedProviderSource,
    provider: &Machine,
    template: &Machine,
    package: Option<semantic_vocabulary::PackageKeyIdentity>,
) -> Vec<Result<language_semantics::provider_declarations::ClosedProviderInstance<String>, String>>
{
    use language_semantics::provider_declarations::ClosedProviderInstance;
    let module = source.module();
    let template_identity = source.typed.machine_callable_identity_text(template);
    let provider_identity = source.typed.machine_callable_identity_text(provider);
    let mut instances = Vec::new();
    for row in &module.provider_instance_requirements {
        if module.identities.text(row.template) != template_identity.as_deref()
            || module.identities.text(row.source_candidate_identity) != provider_identity.as_deref()
            || row.package != package
        {
            continue;
        }
        for required in &row.required_instances {
            instances.push((|| {
                let requirement = module
                    .identities
                    .text(required.requirement_identity)
                    .ok_or("generic provider receipt lost its required identity")?;
                let realization = module
                    .identities
                    .text(required.candidate_identity)
                    .ok_or("generic provider receipt lost its candidate identity")?;
                if !module.provider_candidates.iter().any(|candidate| {
                    candidate.requirement_identity == required.requirement_identity
                        && candidate.candidate_identity == required.candidate_identity
                        && candidate.declaration_application == required.declaration_application
                        && candidate.receiving_application == required.receiving_application
                        && candidate.receiving_region_arguments
                            == required.receiving_region_arguments
                }) {
                    return Err(
                        "generic provider has no accepted exact application refinement".to_owned(),
                    );
                }
                Ok(ClosedProviderInstance {
                    requirement: requirement.to_owned(),
                    realization: realization.to_owned(),
                })
            })());
        }
    }
    if instances.is_empty()
        && !typed_trees::provider_declarations::closed_provider_requirements(
            &source.typed,
            template,
        )
        .is_empty()
    {
        instances.push(Err(
            "generic provider has no accepted instance-requirement receipt".into(),
        ));
    }
    instances
}
