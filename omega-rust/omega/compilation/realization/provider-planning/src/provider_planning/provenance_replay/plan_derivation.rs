// We derive satisfies plans, top-level requirement plans, boundary operator
// plans, and sealed-accessor plans with their provenance (`derive_provider_plans`).
// Plans belong to authored satisfaction declarations rather than specialized
// instances (`is_authored_provider_machine`), and any candidate plan whose row
// realizations reach a known absent target body (`DirectTargetBodyAbsences`) is
// filtered out as a whole map.

use crate::provider_planning::provenance_replay::requirement_identities::{
    exact_installed_external_binding_identity, exact_satisfied_requirement_identity,
    external_provider_binding, inferred_compiler_leaf_binding, realization_machine_identity,
};
use crate::provider_planning::provenance_replay::{
    DerivedProviderPlan, ProviderPlanProvenance, ProviderRowRealization, ProviderSchemaDeclaration,
    SelectedTargetMachineOrigin, satisfies_plan_name,
};
use crate::provider_planning::source_provider_selections::{
    provider_type_package_identity, provider_type_path, provider_type_symbol,
};
use crate::provider_planning::trait_name_text;
use crate::provider_planning::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema};
use effects::provider_plan::{ProviderIdentityId, ProviderIdentityTable};
use language_semantics::direct_calls::DirectCallDeclarations;
use symbols::SymbolHandle;

pub(crate) fn derive_provider_plans(
    services: checked_trees::typed_trees::service_declarations::ServiceCatalog<'_>,
    body_declarations: &DirectCallDeclarations<SymbolHandle>,
    calling_plans: &[crate::calling_policy_plans::BoundaryCallingPlanRealization],
    selected_target: Option<&str>,
    evaluated_bindings: Option<&crate::evaluated_via_bindings::EvaluatedViaBindingTable>,
    target_machine_origins: &[SelectedTargetMachineOrigin],
    native_bindings: &[DerivedProviderPlan],
) -> Result<Vec<DerivedProviderPlan>, Vec<diagnostics::Diagnostic>> {
    let context = services.context();
    let typed = context.typed;
    let mut plans: Vec<DerivedProviderPlan> = Vec::new();
    let native_row_requirements: Vec<SymbolHandle> = native_bindings
        .iter()
        .filter(|plan| plan.plan.target == selected_target.unwrap_or_default())
        .flat_map(|plan| plan.provenance.row_requirements.iter().copied())
        .collect();
    for machine in typed.machines() {
        if !is_authored_provider_machine(typed, machine)
            || !crate::target_machines::realizes_machine(context, machine, selected_target)
        {
            continue;
        }
        let origin_package_identity = typed.symbols.symbol_package_identity(machine.symbol);
        let provider_type_package_identity = provider_type_package_identity(typed, machine);
        let provider_type_symbol = provider_type_symbol(typed, machine);
        for clause in checked_trees::typed_trees::provider_declarations::independent_satisfactions(
            typed, machine,
        ) {
            if clause.requirement.is_some()
                && checked_trees::typed_trees::operator::resolve_satisfied_boundary_operator_for_conformance(
                    typed, machine, clause,
                )
                .is_some()
            {
                continue;
            }
            let Some(requirement) = clause.requirement.as_ref() else {
                continue;
            };
            let (row_binding, target_machine_origin) =
                match (machine.supply_mode, clause.external_binding) {
                    (language_semantics::MachineSupplyMode::Boundary, None)
                        if !machine.body_is_present
                            && !clause.via_expression.is_valid()
                            && clause.external_binding_source_span.is_none() =>
                    {
                        if native_row_requirements.contains(&clause.requirement_symbol) {
                            continue;
                        }
                        let Some((binding, origin)) = inferred_compiler_leaf_binding(
                            typed,
                            machine,
                            clause,
                            selected_target,
                            target_machine_origins,
                        ) else {
                            continue;
                        };
                        (DraftRowBinding::Bound(binding), Some(origin))
                    }
                    (language_semantics::MachineSupplyMode::CheckedBody, None)
                        if machine.body_is_present && !clause.via_expression.is_valid() =>
                    {
                        let is_boundary_trait = typed.traits().iter().any(|definition| {
                            definition.is_boundary && definition.symbol == clause.symbol
                        });
                        if !is_boundary_trait {
                            continue;
                        }
                        (checked_adapter_draft(typed, machine), None)
                    }
                    _ => {
                        let Some(binding) = external_realization_row_binding(
                            typed,
                            evaluated_bindings,
                            machine,
                            clause,
                            selected_target,
                        ) else {
                            continue;
                        };
                        (binding, None)
                    }
                };
            let target = selected_target.unwrap_or_default().to_owned();
            let provider_type = provider_type_path(typed, machine).unwrap_or_default();
            let semantic_requirement_identity = exact_satisfied_requirement_identity(
                typed,
                clause.symbol,
                clause.requirement_symbol,
            );
            let requirement_symbol = clause.requirement_symbol;
            if let Some((schema_declaration, schema_trait, schema)) =
                provider_plan_schema_target(services, calling_plans, clause.symbol)
            {
                let plan_name = satisfies_plan_name(&target, &schema_trait, &provider_type);
                let position = derived_plan_position(
                    &mut plans,
                    plan_name,
                    provider_type.clone(),
                    provider_type_package_identity,
                    target.clone(),
                    schema,
                    schema_declaration,
                    provider_type_symbol,
                    origin_package_identity,
                );
                debug_assert_eq!(plans[position].provenance.schema, schema_declaration);
                debug_assert_eq!(
                    plans[position].provenance.provider_type,
                    provider_type_symbol
                );
                let binding = row_binding.resolve(&mut plans[position].plan.identities);
                let requirement_identity = mint_plan_row_identity(
                    &mut plans[position].plan,
                    &semantic_requirement_identity,
                );
                plans[position].plan.rows.push(ProviderPlanRow {
                    method: requirement.as_str().to_owned(),
                    requirement_identity,
                    declared_requirement_identity: requirement_identity,
                    calling_signature: None,
                    requirement_lifetime_partition:
                        language_semantics::type_identity::normalize_requirement_lifetime_partition(
                            &clause.trait_lifetime_arguments,
                        ),
                    admitted_same_stack_bytes: machine.admitted_same_stack_bytes,
                    binding,
                });
                plans[position].provenance.push_authored_row(
                    requirement_symbol,
                    machine.symbol,
                    target_machine_origin.clone(),
                );
            }
        }
    }
    plans.extend(derive_boundary_operator_plans(
        services,
        selected_target,
        evaluated_bindings,
    ));
    plans.extend(derive_top_level_requirement_plans(
        services,
        selected_target,
        evaluated_bindings,
    ));
    let mut join_candidates = plans.clone();
    join_candidates.extend(native_bindings.iter().cloned());
    let named = super::named_provider_plans::derive(
        services,
        calling_plans,
        selected_target,
        &join_candidates,
    );
    if std::env::var_os("OMEGA_PROVIDER_DEBUG").is_some() {
        for plan in &named {
            eprintln!("[prov] named-derived plan={}", plan.plan.name);
        }
    }
    plans.extend(named);
    derive_accessor_plans(services, selected_target, &mut plans)?;
    let absent = provider_selection::DirectTargetBodyAbsences::derive(
        body_declarations,
        selected_target
            .filter(|target| !target.is_empty())
            .unwrap_or_else(|| target::TargetProfile::host().target_name()),
    );
    let debug = std::env::var_os("OMEGA_PROVIDER_DEBUG").is_some();
    plans.retain(|candidate| {
        let dropped = candidate
            .provenance
            .row_realizations
            .iter()
            .filter_map(ProviderRowRealization::authored)
            .any(|machine| absent.contains(&machine));
        if debug && dropped {
            eprintln!("[prov] absent-filtered plan={}", candidate.plan.name);
        }
        !dropped
    });
    Ok(plans)
}

fn derive_top_level_requirement_plans(
    services: checked_trees::typed_trees::service_declarations::ServiceCatalog<'_>,
    selected_target: Option<&str>,
    evaluated_bindings: Option<&crate::evaluated_via_bindings::EvaluatedViaBindingTable>,
) -> Vec<DerivedProviderPlan> {
    let context = services.context();
    let typed = context.typed;
    let mut plans = Vec::<DerivedProviderPlan>::new();
    for machine in typed.machines() {
        if !is_authored_provider_machine(typed, machine)
            || !crate::target_machines::realizes_machine(context, machine, selected_target)
        {
            continue;
        }
        let is_checked_adapter = machine.supply_mode
            == language_semantics::MachineSupplyMode::CheckedBody
            && machine.body_is_present;
        let is_external_leaf = matches!(
            machine.supply_mode,
            language_semantics::MachineSupplyMode::ExternalRealization { .. }
        ) && !machine.body_is_present;
        if !is_checked_adapter && !is_external_leaf {
            continue;
        }
        let Some(provider_type_symbol) = provider_type_symbol(typed, machine) else {
            continue;
        };
        let Some(provider_type) = provider_type_path(typed, machine) else {
            continue;
        };
        let provider_type_package_identity = provider_type_package_identity(typed, machine);
        let origin_package_identity = typed.symbols.symbol_package_identity(machine.symbol);
        for clause in checked_trees::typed_trees::provider_declarations::independent_satisfactions(
            typed, machine,
        ) {
            let Some(requirement) = typed.machines().iter().find(|candidate| {
                candidate.symbol == clause.symbol
                    && clause.requirement_symbol == candidate.symbol
                    && candidate.supply_mode
                        == language_semantics::MachineSupplyMode::TopLevelRequirement
            }) else {
                continue;
            };
            let Some(prepared) = services.declaration(
                ProviderSchemaDeclaration::BoundaryRequirement(requirement.symbol),
            ) else {
                continue;
            };
            let schema = ServiceSchema::from_declaration(prepared.declaration.clone());
            let requirement_identity = typed
                .machine_callable_identity_text(requirement)
                .unwrap_or_default();
            let [schema_method] = schema.declaration.methods.as_slice() else {
                continue;
            };
            if requirement_identity.is_empty()
                || schema.identity_text(schema_method.requirement_identity) != requirement_identity
            {
                continue;
            }
            let binding = match (machine.supply_mode, clause.external_binding) {
                (language_semantics::MachineSupplyMode::CheckedBody, None)
                    if machine.body_is_present && !clause.via_expression.is_valid() =>
                {
                    checked_adapter_draft(typed, machine)
                }
                _ if !machine.body_is_present => {
                    let Some(binding) = external_realization_row_binding(
                        typed,
                        evaluated_bindings,
                        machine,
                        clause,
                        selected_target,
                    ) else {
                        continue;
                    };
                    binding
                }
                _ => continue,
            };
            let target = selected_target.unwrap_or_default().to_owned();
            let plan_name = satisfies_plan_name(&target, trait_name_text(&schema), &provider_type);
            let position = derived_plan_position(
                &mut plans,
                plan_name,
                provider_type.clone(),
                provider_type_package_identity,
                target.clone(),
                schema.clone(),
                ProviderSchemaDeclaration::BoundaryRequirement(requirement.symbol),
                Some(provider_type_symbol),
                origin_package_identity,
            );
            let binding = binding.resolve(&mut plans[position].plan.identities);
            let requirement_identity =
                mint_plan_row_identity(&mut plans[position].plan, &requirement_identity);
            plans[position].plan.rows.push(ProviderPlanRow {
                method: schema_method.name.clone(),
                requirement_identity,
                declared_requirement_identity: requirement_identity,
                calling_signature: None,
                requirement_lifetime_partition: Vec::new(),
                admitted_same_stack_bytes: machine.admitted_same_stack_bytes,
                binding,
            });
            plans[position]
                .provenance
                .push_authored_row(requirement.symbol, machine.symbol, None);
        }
    }
    plans
}

fn provider_plan_schema_target(
    services: checked_trees::typed_trees::service_declarations::ServiceCatalog<'_>,
    calling_plans: &[crate::calling_policy_plans::BoundaryCallingPlanRealization],
    satisfied_trait_symbol: symbols::SymbolHandle,
) -> Option<(ProviderSchemaDeclaration, String, ServiceSchema)> {
    let source = ProviderSchemaDeclaration::BoundaryTrait(satisfied_trait_symbol);
    let prepared = services.declaration(source)?;
    let schema = crate::service_schema::attach_calling(&prepared, calling_plans);
    Some((source, trait_name_text(&schema).to_owned(), schema))
}

fn derive_boundary_operator_plans(
    services: checked_trees::typed_trees::service_declarations::ServiceCatalog<'_>,
    selected_target: Option<&str>,
    evaluated_bindings: Option<&crate::evaluated_via_bindings::EvaluatedViaBindingTable>,
) -> Vec<DerivedProviderPlan> {
    let context = services.context();
    let typed = context.typed;
    let mut plans = Vec::<DerivedProviderPlan>::new();
    for machine in typed.machines() {
        if !is_authored_provider_machine(typed, machine)
            || !crate::target_machines::realizes_machine(context, machine, selected_target)
        {
            continue;
        }
        let origin_package_identity = typed.symbols.symbol_package_identity(machine.symbol);
        let provider_type_package_identity = provider_type_package_identity(typed, machine);
        let provider_type_symbol = provider_type_symbol(typed, machine);
        for clause in checked_trees::typed_trees::provider_declarations::independent_satisfactions(
            typed, machine,
        ) {
            if clause.requirement.is_none() {
                continue;
            }
            let Some(operator) =
                checked_trees::typed_trees::operator::resolve_satisfied_boundary_operator_for_conformance(
                    typed, machine, clause,
                )
            else {
                continue;
            };
            if typed.machine(operator.symbol).is_some() {
                continue;
            }
            let binding = match (machine.supply_mode, clause.external_binding) {
                (language_semantics::MachineSupplyMode::CheckedBody, None)
                    if machine.body_is_present && !clause.via_expression.is_valid() =>
                {
                    checked_adapter_draft(typed, machine)
                }
                _ => {
                    let Some(binding) = external_realization_row_binding(
                        typed,
                        evaluated_bindings,
                        machine,
                        clause,
                        selected_target,
                    ) else {
                        continue;
                    };
                    binding
                }
            };
            let Some(prepared) =
                services.declaration(ProviderSchemaDeclaration::BoundaryOperator(operator.symbol))
            else {
                continue;
            };
            let schema = ServiceSchema::from_declaration(prepared.declaration.clone());
            let target = selected_target.unwrap_or_default().to_owned();
            let provider_type = provider_type_path(typed, machine).unwrap_or_default();
            let plan_name = satisfies_plan_name(&target, trait_name_text(&schema), &provider_type);
            let position = derived_plan_position(
                &mut plans,
                plan_name,
                provider_type,
                provider_type_package_identity,
                target,
                schema.clone(),
                ProviderSchemaDeclaration::BoundaryOperator(operator.symbol),
                provider_type_symbol,
                origin_package_identity,
            );
            debug_assert_eq!(
                plans[position].provenance.schema,
                ProviderSchemaDeclaration::BoundaryOperator(operator.symbol)
            );
            debug_assert_eq!(
                plans[position].provenance.provider_type,
                provider_type_symbol
            );
            let binding = binding.resolve(&mut plans[position].plan.identities);
            let requirement_identity = mint_plan_row_identity(
                &mut plans[position].plan,
                schema.identity_text(schema.declaration.methods[0].requirement_identity),
            );
            plans[position].plan.rows.push(ProviderPlanRow {
                method: "realize".to_owned(),
                requirement_identity,
                declared_requirement_identity: requirement_identity,
                calling_signature: None,
                requirement_lifetime_partition: Vec::new(),
                admitted_same_stack_bytes: machine.admitted_same_stack_bytes,
                binding,
            });
            plans[position]
                .provenance
                .push_authored_row(operator.symbol, machine.symbol, None);
        }
    }
    plans
}

enum DraftRowBinding {
    Bound(ProviderBinding),
    CheckedAdapter {
        machine_identity: String,
        machine_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    },
}

impl DraftRowBinding {
    fn resolve(&self, identities: &mut ProviderIdentityTable) -> ProviderBinding {
        match self {
            Self::Bound(binding) => binding.clone(),
            Self::CheckedAdapter {
                machine_identity,
                machine_package_identity,
            } => ProviderBinding::CheckedAdapter {
                machine_identity: identities.intern(machine_identity),
                machine_package_identity: *machine_package_identity,
            },
        }
    }
}

fn is_authored_provider_machine(
    typed: &checked_trees::typed_trees::TypedTrees,
    machine: &checked_trees::typed_trees::machine::Machine,
) -> bool {
    !typed
        .machine_specializations
        .iter()
        .any(|receipt| receipt.instance == machine.symbol)
}

fn checked_adapter_draft(
    typed: &checked_trees::typed_trees::TypedTrees,
    machine: &checked_trees::typed_trees::machine::Machine,
) -> DraftRowBinding {
    DraftRowBinding::CheckedAdapter {
        machine_identity: typed
            .machine_callable_identity_text(machine)
            .unwrap_or_default(),
        machine_package_identity: typed.symbols.symbol_package_identity(machine.symbol),
    }
}

fn evaluated_via_binding(
    evaluated_bindings: Option<&crate::evaluated_via_bindings::EvaluatedViaBindingTable>,
    machine: &checked_trees::typed_trees::machine::Machine,
    clause: &checked_trees::typed_trees::machine::TraitConformance,
    selected_target: Option<&str>,
) -> Option<ProviderBinding> {
    let table = evaluated_bindings?;
    let row = table.exact(machine.symbol, clause.symbol, clause.requirement_symbol)?;
    // The row must have been evaluated for the scope deriving it. A
    // target-authored leaf is evaluated under its own target, so its row
    // serves that target's scope whichever target this operation emits; an
    // untargeted leaf is evaluated under the table's target only, so it
    // cannot serve another scope. Requiring the table's target as well would
    // drop every authored leaf from each declared but unemitted scope, such
    // as windows_x86_64's `via` ProcessExit.
    if selected_target != Some(row.evaluated().target().target_name()) {
        return None;
    }
    Some(row.evaluated().provider_binding())
}

fn external_realization_row_binding(
    typed: &checked_trees::typed_trees::TypedTrees,
    evaluated_bindings: Option<&crate::evaluated_via_bindings::EvaluatedViaBindingTable>,
    machine: &checked_trees::typed_trees::machine::Machine,
    clause: &checked_trees::typed_trees::machine::TraitConformance,
    selected_target: Option<&str>,
) -> Option<DraftRowBinding> {
    let language_semantics::MachineSupplyMode::ExternalRealization {
        binding: supply_binding,
        mechanism: supply_mechanism,
    } = machine.supply_mode
    else {
        return None;
    };
    if clause.via_expression.is_valid() {
        if supply_binding.is_some()
            || supply_mechanism.is_some()
            || clause.external_binding.is_some()
            || machine.body_is_present
        {
            return None;
        }
        return Some(DraftRowBinding::Bound(evaluated_via_binding(
            evaluated_bindings,
            machine,
            clause,
            selected_target,
        )?));
    }
    let binding = exact_installed_external_binding_identity(
        typed,
        machine,
        clause.external_binding?,
        clause.name.as_str(),
        clause.requirement.as_ref().map_or("", |name| name.as_str()),
    )?;
    let dispatch_table = match binding {
        language_semantics::ExternalBindingIdentity::VtableField { .. }
        | language_semantics::ExternalBindingIdentity::TableFunction { .. } => {
            crate::dispatch_tables::dispatch_table(typed, machine)
                .map(|table| table.path(typed))
                .unwrap_or_default()
        }
        _ => String::new(),
    };
    Some(DraftRowBinding::Bound(external_provider_binding(
        binding,
        &dispatch_table,
        &realization_machine_identity(typed, machine),
        match binding {
            language_semantics::ExternalBindingIdentity::VtableField { field }
            | language_semantics::ExternalBindingIdentity::TableFunction { field } => {
                crate::dispatch_tables::field_storage(typed, machine, field)
            }
            _ => Err("external binding does not read a table field".into()),
        },
    )))
}

fn mint_plan_row_identity(
    plan: &mut ProviderPlan,
    requirement_identity: &str,
) -> ProviderIdentityId {
    plan.identities.intern(requirement_identity)
}

#[allow(clippy::too_many_arguments)]
fn derived_plan_position(
    plans: &mut Vec<DerivedProviderPlan>,
    plan_name: String,
    provider_type: String,
    provider_type_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    target: String,
    schema: ServiceSchema,
    schema_declaration: ProviderSchemaDeclaration,
    provider_type_symbol: Option<symbols::SymbolHandle>,
    origin_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
) -> usize {
    plans
        .iter()
        .position(|derived| {
            derived.plan.name == plan_name
                && derived.plan.provider_type_package_identity == provider_type_package_identity
                && derived.plan.origin_package_identity == origin_package_identity
                && derived.provenance.schema == schema_declaration
                && derived.provenance.provider_type == provider_type_symbol
        })
        .unwrap_or_else(|| {
            plans.push(DerivedProviderPlan {
                plan: ProviderPlan {
                    name: plan_name,
                    provider_application: Default::default(),
                    provider_type,
                    provider_type_package_identity,
                    target,
                    identities: ProviderIdentityTable::default(),
                    schema,
                    rows: Vec::new(),
                    origin_package_identity,
                    origin_package: String::new(),
                    digest_memo: Default::default(),
                },
                provenance: ProviderPlanProvenance {
                    schema: schema_declaration,
                    provider_type: provider_type_symbol,
                    named_application: None,
                    row_requirements: Vec::new(),
                    row_realizations: Vec::new(),
                    row_target_machine_origins: Vec::new(),
                    row_accessors: Vec::new(),
                },
            });
            plans.len() - 1
        })
}

fn derive_accessor_plans(
    services: checked_trees::typed_trees::service_declarations::ServiceCatalog<'_>,
    selected_target: Option<&str>,
    plans: &mut Vec<DerivedProviderPlan>,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let context = services.context();
    let Some(target) = selected_target else {
        return Ok(());
    };
    let typed = context.typed;
    for view in &typed.access_view_plans {
        for field in &view.fields {
            for operation in &field.accessor_targets {
                if !context.contains(operation.machine_symbol) {
                    continue;
                }
                let Some(accessor) =
                    crate::SealedAccessor::prepared(services, operation.machine_symbol)
                        .map_err(|error| vec![error])?
                else {
                    continue;
                };
                let schema = accessor
                    .prepared_schema(services)
                    .map_err(|error| vec![error])?;
                let machine = typed.machine(accessor.machine()).ok_or_else(|| {
                    vec![diagnostics::Diagnostic::error(
                        "placed catalog lost its generated machine",
                    )]
                })?;
                let requirement =
                    typed
                        .machine_callable_identity_text(machine)
                        .ok_or_else(|| {
                            vec![diagnostics::Diagnostic::error(
                                "placed catalog lost its exact callable identity",
                            )]
                        })?;
                let provider = typed
                    .normalized_hermetic_symbol_identity(field.accessor_data_symbol)
                    .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
                let position = derived_plan_position(
                    plans,
                    satisfies_plan_name(target, &requirement, &provider),
                    provider,
                    typed
                        .symbols
                        .symbol_package_identity(field.accessor_data_symbol),
                    target.to_owned(),
                    schema,
                    ProviderSchemaDeclaration::SealedAccessor(accessor.machine()),
                    Some(field.accessor_data_symbol),
                    typed.symbols.symbol_package_identity(machine.symbol),
                );
                let requirement_identity =
                    mint_plan_row_identity(&mut plans[position].plan, &requirement);
                plans[position].plan.rows.push(ProviderPlanRow {
                    method: operation.operation.clone(),
                    requirement_identity,
                    declared_requirement_identity: requirement_identity,
                    calling_signature: None,
                    requirement_lifetime_partition: Vec::new(),
                    admitted_same_stack_bytes: machine.admitted_same_stack_bytes,
                    binding: ProviderBinding::CompilerIntrinsic {
                        machine: accessor
                            .prepared_binding_identity(services)
                            .map_err(|error| vec![error])?,
                    },
                });
                let provenance = &mut plans[position].provenance;
                provenance.row_requirements.push(machine.symbol);
                provenance
                    .row_realizations
                    .push(ProviderRowRealization::Authored(machine.symbol));
                provenance.row_target_machine_origins.push(None);
                provenance.row_accessors.push(Some(accessor));
            }
        }
    }
    Ok(())
}
