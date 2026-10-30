//! Derive whole named-provider plans from checked source applications.
//! Declaration-level public regions stay universal. Captured applications own
//! their complete closed telescope; no body or slot name reconstructs it.
use super::requirement_identities::exact_satisfied_requirement_identity;
use super::{
    DerivedProviderPlan, ProviderPlanProvenance, ProviderRowRealization, ProviderSchemaDeclaration,
    satisfies_plan_name,
};
use crate::ProviderPlanningContext;
use crate::provider_planning::{ProviderBinding, ProviderPlan, ProviderPlanRow};
use build_time_evaluation::provider_declarations::{
    prepared_named_provider_applications, prepared_named_provider_member,
};
use effects::provider_plan::ProviderIdentityTable;

pub(super) fn derive(
    context: ProviderPlanningContext<'_>,
    calling_plans: &[crate::calling_policy_plans::BoundaryCallingPlanRealization],
    selected_target: Option<&str>,
    candidates: &[DerivedProviderPlan],
) -> Vec<DerivedProviderPlan> {
    let typed = context.typed;
    prepared_named_provider_applications(context)
        .into_iter()
        .filter_map(|application| {
            let implementation = application.declaration;
            let rows = &implementation.rows;
            let definition = typed.traits().iter().find(|definition| {
                definition.symbol == implementation.trait_symbol
                    && definition.is_boundary
                    && context.contains(definition.symbol)
            })?;
            let arguments = typed
                .type_reference_table
                .type_reference_handles(implementation.arguments);
            let schema = crate::service_schema::from_typed_application(
                context,
                calling_plans,
                application.scope.clone(),
                arguments,
            )?;
            let provider_type = typed.symbols.display_path(implementation.symbol, "::");
            let package = typed.symbols.symbol_package_identity(implementation.symbol);
            let origin_package = typed
                .symbols
                .symbol_package_identity(rows.first()?.realization_machine);
            let target = selected_target.unwrap_or_default().to_owned();
            let mut plan = ProviderPlan {
                name: satisfies_plan_name(
                    &target,
                    schema.identity_text(schema.trait_name),
                    &provider_type,
                ),
                provider_type,
                provider_type_package_identity: package,
                provider_application: Default::default(),
                target,
                identities: ProviderIdentityTable::default(),
                schema,
                rows: Vec::new(),
                origin_package_identity: origin_package,
                origin_package: String::new(),
                digest_memo: Default::default(),
            };
            if let Some(identity) = &application.identity {
                plan.provider_application = plan.identities.intern(identity);
                plan.name.push_str("::");
                plan.name.push_str(identity);
            }
            let mut provenance = ProviderPlanProvenance {
                schema: ProviderSchemaDeclaration::BoundaryTrait(definition.symbol),
                provider_type: Some(implementation.symbol),
                row_requirements: Vec::new(),
                row_realizations: Vec::new(),
                row_target_machine_origins: Vec::new(),
                row_placed_accessors: Vec::new(),
            };
            for row in rows {
                // The row binds the entry of the machine resolution named; a
                // family head's entry stands for every member's.
                let named = typed.machine_by_symbol(row.realization_machine)?;
                if typed.machine_states(named).first()?.symbol != row.realization_state {
                    return None;
                }
                let realization = if application.application.is_some() {
                    prepared_named_provider_member(typed, &application, row.realization_machine)?
                } else {
                    row.realization_machine
                };
                let machine = crate::target_machines::realized_row_machine(
                    context,
                    realization,
                    selected_target,
                )?;
                if typed.symbols.symbol_package_identity(machine.symbol) != origin_package {
                    return None;
                }
                let identity = exact_satisfied_requirement_identity(
                    typed,
                    row.declaring_trait,
                    row.requirement,
                );
                let lifetime_arguments =
                    crate::provider_identity::named_requirement_lifetime_arguments(
                        typed,
                        implementation,
                        row.declaring_trait,
                    )?;
                let (binding, lifetime_partition, origin) = if machine.supply_mode.is_checked_body()
                    && machine.body_is_present
                {
                    (
                        ProviderBinding::CheckedAdapter {
                            machine_identity: plan
                                .identities
                                .intern(&typed.machine_callable_identity_text(machine)?),
                            machine_package_identity: typed
                                .symbols
                                .symbol_package_identity(machine.symbol),
                        },
                        language_semantics::type_identity::normalize_requirement_lifetime_partition(
                            lifetime_arguments,
                        ),
                        None,
                    )
                } else {
                    // Foreign candidates belong to the selected target member,
                    // while the authored map can name its portable family head.
                    // Join the resolved identity, just as checked adapters do.
                    let mut matched = Vec::new();
                    for candidate in candidates {
                        for (index, binding) in candidate.plan.rows.iter().enumerate() {
                            if candidate.provenance.row_requirements.get(index)
                                == Some(&row.requirement)
                                && candidate
                                    .provenance
                                    .row_realizations
                                    .get(index)
                                    .and_then(ProviderRowRealization::authored)
                                    == Some(machine.symbol)
                                && candidate.plan.identities.text(binding.requirement_identity)
                                    == identity.as_str()
                            {
                                matched.push((
                                    candidate,
                                    binding,
                                    candidate.provenance.row_target_machine_origins.get(index)?,
                                ));
                            }
                        }
                    }
                    let (candidate, binding, origin) = *matched.first()?;
                    if matched.iter().any(|(_, other, other_origin)| {
                        *other != binding || *other_origin != origin
                    }) {
                        return None;
                    }
                    let origin = origin.clone();
                    let bound = match &binding.binding {
                        ProviderBinding::CheckedAdapter {
                            machine_identity,
                            machine_package_identity,
                        } => ProviderBinding::CheckedAdapter {
                            machine_identity: plan
                                .identities
                                .intern(candidate.plan.identity_text(*machine_identity)),
                            machine_package_identity: *machine_package_identity,
                        },
                        bound => bound.clone(),
                    };
                    (
                        bound,
                        binding.requirement_lifetime_partition.clone(),
                        origin,
                    )
                };
                let requirement_identity = plan.identities.intern(&identity);
                plan.rows.push(ProviderPlanRow {
                    method: row.requirement_name.as_str().to_owned(),
                    requirement_identity,
                    requirement_lifetime_partition: lifetime_partition,
                    admitted_same_stack_bytes: machine.admitted_same_stack_bytes,
                    binding,
                });
                provenance.row_requirements.push(row.requirement);
                provenance
                    .row_realizations
                    .push(ProviderRowRealization::Authored(machine.symbol));
                provenance.row_target_machine_origins.push(origin);
                provenance.row_placed_accessors.push(None);
            }
            Some(DerivedProviderPlan { plan, provenance })
        })
        .collect()
}
