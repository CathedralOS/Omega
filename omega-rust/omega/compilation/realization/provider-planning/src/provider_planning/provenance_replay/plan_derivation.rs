//! Deriving satisfies plans, top-level requirement plans and boundary
//! operator plans with their provenance.
//!
//! Provider paths use the same complete nominal projection as Build selection
//! and Terminal candidate emission. They do not replace declaration custody:
//! grouping/provenance retain exact provider symbols, packages and normalized
//! callable identities, which independent replay must still rejoin.
//!
//! Plans belong to authored satisfaction declarations, not the executable
//! instances specialization mints from them. All requirement schema kinds use
//! that ownership rule; the closed-requirement roster retains and joins every
//! executable tuple separately, without inflating original schema coverage.

use crate::ProviderPlanningContext;
use crate::provider_planning::operator_provider_evidence::{
    provider_type_package_identity, provider_type_symbol,
};
use crate::provider_planning::provenance_replay::requirement_identities::{
    exact_installed_external_binding_identity, exact_satisfied_requirement_identity,
    external_provider_binding, inferred_compiler_leaf_binding, realization_machine_identity,
};
use crate::provider_planning::provenance_replay::{
    DerivedProviderPlan, ProviderPlanCatalog, ProviderPlanProvenance, ProviderSchemaDeclaration,
    SelectedTargetMachineOrigin, UnavailableProviderCandidate, satisfies_plan_name,
};
use crate::provider_planning::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema};
use effects::provider_plan::{ProviderIdentityId, ProviderIdentityTable};

pub(crate) fn derive_provider_plans(
    context: ProviderPlanningContext<'_>,
    calling_plans: &[crate::calling_policy_plans::BoundaryCallingPlanRealization],
    selected_target: Option<&str>,
    evaluated_bindings: Option<&crate::evaluated_via_bindings::EvaluatedViaBindingTable>,
    target_machine_origins: &[SelectedTargetMachineOrigin],
) -> ProviderPlanCatalog {
    let typed = context.typed;
    let mut plans: Vec<DerivedProviderPlan> = Vec::new();
    // Source edges own rows; executable copies retain their outer application
    // separately. Member specialization must not multiply universal rows, while
    // different complete outer applications remain different provider schemas.
    let mut source_applications: Vec<SourceProviderApplication<'_>> = Vec::new();
    // Typed trees retain every authored body and its conformances. This
    // transitional settlement derives only the requested realization's rows;
    // semantic declaration queries must never perform that selection for us.
    for machine in typed.machines() {
        if !crate::target_machines::realizes_machine(context, machine, selected_target) {
            continue;
        }
        let provider_type_package_identity = provider_type_package_identity(typed, machine);
        let provider_type_symbol = provider_type_symbol(typed, machine);
        for (ordinal, clause) in typed.machine_trait_conformances(machine).iter().enumerate() {
            if clause.requirement.is_some()
                && checked_trees::typed_trees::operator::resolve_satisfied_boundary_operator_for_conformance(
                    typed, machine, clause,
                )
                .is_some()
            {
                // Exact boundary-operator requirements use one overloaded
                // signature per provider slot; derive them below rather than
                // manufacturing an empty boundary-trait schema here.
                continue;
            }
            // A bodyless leaf carries `via`; a CHECKED ADAPTER is an
            // ordinary machine with a body and a requirement-named
            // satisfies edge (no via). Both contribute rows; whole-trait
            // conformances (no requirement) are the trait system's
            // ordinary business and derive nothing here.
            let Some(requirement) = clause.requirement.as_ref() else {
                continue;
            };
            for source in typed
                .machine_conformance_sources(machine, machine.satisfies.handle_at(ordinal as u32))
            {
                let family = source.family;
                let origin_package_identity = typed.symbols.symbol_package_identity(family.symbol);
                let original = source.edge;
                if source_applications.iter().any(|application| {
                    application.edge == source.source
                        && application.provider == provider_type_symbol
                        && application.scope.same_application(typed, &source.scope)
                }) {
                    continue;
                }
                let (row_binding, target_machine_origin) =
                    match (family.supply_mode, original.external_binding) {
                        (language_semantics::MachineSupplyMode::Boundary, None)
                            if !family.body_is_present
                                && !original.via_expression.is_valid()
                                && original.external_binding_source_span.is_none() =>
                        {
                            let Some((binding, origin)) = inferred_compiler_leaf_binding(
                                typed,
                                family,
                                original,
                                selected_target,
                                target_machine_origins,
                            ) else {
                                continue;
                            };
                            (DraftRowBinding::Bound(binding), Some(origin))
                        }
                        (
                            language_semantics::MachineSupplyMode::ExternalRealization { .. },
                            Some(conformance_binding),
                        ) if !original.via_expression.is_valid() => {
                            let Some(binding) = exact_installed_external_binding_identity(
                                typed,
                                family,
                                conformance_binding,
                                original.name.as_str(),
                                requirement.as_str(),
                            ) else {
                                continue;
                            };
                            let binding = external_provider_binding(
                                binding,
                                &typed.attached_data_path(machine).unwrap_or_default(),
                                &realization_machine_identity(typed, family),
                            );
                            (DraftRowBinding::Bound(binding), None)
                        }
                        (
                            language_semantics::MachineSupplyMode::ExternalRealization {
                                binding: None,
                                mechanism: None,
                            },
                            None,
                        ) if !family.body_is_present && original.via_expression.is_valid() => {
                            let Some(binding) = evaluated_via_binding(
                                evaluated_bindings,
                                family,
                                original,
                                selected_target,
                            ) else {
                                continue;
                            };
                            (DraftRowBinding::Bound(binding), None)
                        }
                        (language_semantics::MachineSupplyMode::CheckedBody, None)
                            if family.body_is_present && !original.via_expression.is_valid() =>
                        {
                            // A CHECKED ADAPTER derives a plan row only over a
                            // BOUNDARY trait (a service schema). A plain trait's
                            // conformance -- including its service-reach ceiling -- is the
                            // existing trait machinery's business (the decision-20
                            // admission fixtures pin it) and derives nothing here.
                            let is_boundary_trait = typed.traits().iter().any(|definition| {
                                definition.is_boundary && definition.symbol == original.symbol
                            });
                            if !is_boundary_trait {
                                continue;
                            }
                            (checked_adapter_draft(typed, family), None)
                        }
                        _ => continue, // refused elsewhere (via rungs)
                    };
                let target = selected_target.unwrap_or_default().to_owned();
                let provider_type = typed.attached_data_path(machine).unwrap_or_default();
                let semantic_requirement_identity = exact_satisfied_requirement_identity(
                    typed,
                    original.symbol,
                    original.requirement_symbol,
                );
                let requirement_symbol = original.requirement_symbol;
                if let Some((schema_declaration, schema_trait, schema)) =
                    provider_plan_schema_target(context, calling_plans, source.scope.clone())
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
                    requirement_lifetime_partition:
                        language_semantics::type_identity::normalize_requirement_lifetime_partition(
                            &original.trait_lifetime_arguments,
                        ),
                    binding,
                });
                    plans[position]
                        .provenance
                        .row_requirements
                        .push(requirement_symbol);
                    plans[position]
                        .provenance
                        .row_realizations
                        .push(family.symbol);
                    plans[position]
                        .provenance
                        .row_receiver_scopes
                        .push(Some(source.scope.retain_payload(typed)));
                    plans[position]
                        .provenance
                        .row_target_machine_origins
                        .push(target_machine_origin.clone());
                    source_applications.push(SourceProviderApplication {
                        edge: source.source,
                        provider: provider_type_symbol,
                        scope: source.scope,
                    });
                }
            }
        }
    }
    plans.extend(derive_boundary_operator_plans(
        context,
        selected_target,
        evaluated_bindings,
    ));
    plans.extend(derive_top_level_requirement_plans(
        context,
        selected_target,
        evaluated_bindings,
    ));
    let named = derive_named_provider_plans(context, calling_plans, selected_target, &plans);
    plans.extend(named.plans);
    ProviderPlanCatalog {
        plans,
        unavailable: named.unavailable,
    }
}

/// Construction scratch for source-row grouping. Handles identify authored
/// edges and providers; the retained scope distinguishes their outer tuple.
struct SourceProviderApplication<'program> {
    edge: arena::Handle<checked_trees::typed_trees::machine::TraitConformance>,
    provider: Option<symbols::SymbolHandle>,
    scope: checked_trees::typed_trees::trait_definition::AppliedTraitScope<'program>,
}

fn derive_named_provider_plans(
    context: ProviderPlanningContext<'_>,
    calling_plans: &[crate::calling_policy_plans::BoundaryCallingPlanRealization],
    selected_target: Option<&str>,
    candidates: &[DerivedProviderPlan],
) -> ProviderPlanCatalog {
    let typed = context.typed;
    let mut unavailable = Vec::new();
    let plans = typed
        .conformances()
        .iter()
        .filter_map(|implementation| {
            if !crate::provider_identity::eligible(implementation)
                || !context.contains(implementation.symbol)
            {
                return None;
            }
            let definition = typed.traits().iter().find(|definition| {
                definition.symbol == implementation.trait_symbol
                    && definition.is_boundary
                    && context.contains(definition.symbol)
            })?;
            let members =
                checked_trees::typed_trees::static_applications::boundary_interface_members(
                    typed,
                    definition.symbol,
                )?;
            // The original map also owns ordinary parent contract evidence.
            // Only declared boundary members contribute callable plan rows.
            let rows = implementation
                .rows
                .iter()
                .filter(|row| members.contains(&(row.declaring_trait, row.requirement)))
                .collect::<Vec<_>>();
            let arguments = typed
                .type_reference_table
                .type_reference_handles(implementation.arguments);
            let receiver_scope =
                checked_trees::typed_trees::trait_definition::AppliedTraitScope::from_conformance(
                    typed,
                    implementation,
                )?;
            let receiver_payload = receiver_scope.retain_payload(typed);
            let schema = crate::service_schema::from_typed_application(
                context,
                calling_plans,
                receiver_scope.clone(),
                arguments,
            )?;
            let provider_type = typed.symbols.display_path(implementation.symbol, "::");
            let package = typed.symbols.symbol_package_identity(implementation.symbol);
            let origin_package = typed
                .symbols
                .symbol_package_identity(rows.first()?.realization_machine);
            let target = selected_target.unwrap_or_default().to_owned();
            let unavailable_schema = schema.clone();
            let unavailable_provider_type = provider_type.clone();
            let unavailable_target = target.clone();
            let mut row_refusal = None;
            let result = (|| {
                let mut plan = ProviderPlan {
                    name: satisfies_plan_name(
                        &target,
                        schema.identity_text(schema.trait_name),
                        &provider_type,
                    ),
                    provider_type,
                    provider_type_package_identity: package,
                    target,
                    identities: ProviderIdentityTable::default(),
                    schema,
                    rows: Vec::new(),
                    origin_package_identity: origin_package,
                    origin_package: String::new(),
                    digest_memo: Default::default(),
                };
                let mut provenance = ProviderPlanProvenance {
                    schema: ProviderSchemaDeclaration::BoundaryTrait(definition.symbol),
                    provider_type: Some(implementation.symbol),
                    row_requirements: Vec::new(),
                    row_realizations: Vec::new(),
                    row_receiver_scopes: Vec::new(),
                    row_target_machine_origins: Vec::new(),
                };
                for row in rows {
                    // The source row continues to name its portable family. Only
                    // this Omega realization chooses the physical member whose
                    // existing checked/evaluated binding can fill the map's slot.
                    let machine = match crate::target_machines::named_row_realization(context, row, selected_target) {
                        Ok(machine) => machine,
                        Err(reason) => { row_refusal = Some(reason); return None; }
                    };
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
                        let mut matched = Vec::new();
                        for candidate in candidates {
                            for (index, binding) in candidate.plan.rows.iter().enumerate() {
                                if candidate.provenance.row_requirements.get(index)
                                    == Some(&row.requirement)
                                    && candidate.provenance.row_realizations.get(index)
                                        == Some(&machine.symbol)
                                    && candidate.plan.identities.text(binding.requirement_identity)
                                        == identity.as_str()
                                    && candidate
                                        .provenance
                                        .row_receiver_scopes
                                        .get(index)
                                        .and_then(|scope| scope.as_ref())
                                        .and_then(|scope| scope.bind(typed))
                                        .and_then(|scope| {
                                            scope.declaring_scope(typed, row.declaring_trait)
                                        })
                                        .zip(receiver_scope.declaring_scope(typed, row.declaring_trait))
                                        .is_some_and(|(candidate_scope, named_scope)| {
                                            candidate_scope.same_application(typed, &named_scope)
                                        })
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
                    let requirement_identity = mint_plan_row_identity(&mut plan, &identity);
                    plan.rows.push(ProviderPlanRow {
                        method: row.requirement_name.as_str().to_owned(),
                        requirement_identity,
                        requirement_lifetime_partition: lifetime_partition,
                        binding,
                    });
                    provenance.row_requirements.push(row.requirement);
                    provenance.row_realizations.push(machine.symbol);
                    provenance
                        .row_receiver_scopes
                        .push(Some(receiver_payload.clone()));
                    provenance.row_target_machine_origins.push(origin);
                }
                Some(DerivedProviderPlan { plan, provenance })
            })();
            if result.is_none() {
                unavailable.push(UnavailableProviderCandidate {
                    reason: row_refusal.unwrap_or_else(|| format!(
                        "the original complete provider map cannot derive every checked binding for target `{}`",
                        unavailable_target,
                    )),
                    schema: unavailable_schema,
                    declaration: ProviderSchemaDeclaration::BoundaryTrait(definition.symbol),
                    provider_type: unavailable_provider_type,
                    provider_package: package,
                    original_map: implementation.clone(),
                    receiver_scope: receiver_payload.clone(),
                    target: unavailable_target
                });
            }
            result
        })
        .collect();
    ProviderPlanCatalog { plans, unavailable }
}

fn derive_top_level_requirement_plans(
    context: ProviderPlanningContext<'_>,
    selected_target: Option<&str>,
    evaluated_bindings: Option<&crate::evaluated_via_bindings::EvaluatedViaBindingTable>,
) -> Vec<DerivedProviderPlan> {
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
        let Some(provider_type) = typed.attached_data_path(machine) else {
            continue;
        };
        let provider_type_package_identity = provider_type_package_identity(typed, machine);
        let origin_package_identity = typed.symbols.symbol_package_identity(machine.symbol);
        for clause in typed.machine_trait_conformances(machine) {
            // The authored satisfies edge names the requirement declaration
            // the provider claims: for a generic token-machine template the
            // row retains that authored template identity because provider
            // plans are selected before specialization settles the closed
            // applications. The selected-requirement specialization roster
            // is the exact join to each closed instance Terminal Psi mints;
            // derivation neither aliases the closed requirement to its
            // template nor trusts an equal runtime signature.
            let Some(requirement) = typed.machines().iter().find(|candidate| {
                candidate.symbol == clause.symbol
                    && clause.requirement_symbol == candidate.symbol
                    && candidate.supply_mode
                        == language_semantics::MachineSupplyMode::TopLevelRequirement
            }) else {
                continue;
            };
            let Some(schema) =
                crate::service_schema::from_typed_boundary_requirement(context, requirement)
            else {
                continue;
            };
            let requirement_identity = typed
                .machine_callable_identity_text(requirement)
                .unwrap_or_default();
            let [schema_method] = schema.methods.as_slice() else {
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
                (
                    language_semantics::MachineSupplyMode::ExternalRealization { .. },
                    Some(conformance_binding),
                ) if !machine.body_is_present && !clause.via_expression.is_valid() => {
                    let Some(binding) = exact_installed_external_binding_identity(
                        typed,
                        machine,
                        conformance_binding,
                        clause.name.as_str(),
                        clause
                            .requirement
                            .as_ref()
                            .map(|name| name.as_str())
                            .unwrap_or_default(),
                    ) else {
                        continue;
                    };
                    DraftRowBinding::Bound(external_provider_binding(
                        binding,
                        &provider_type,
                        &realization_machine_identity(typed, machine),
                    ))
                }
                (
                    language_semantics::MachineSupplyMode::ExternalRealization {
                        binding: None,
                        mechanism: None,
                    },
                    None,
                ) if !machine.body_is_present && clause.via_expression.is_valid() => {
                    let Some(binding) =
                        evaluated_via_binding(evaluated_bindings, machine, clause, selected_target)
                    else {
                        continue;
                    };
                    DraftRowBinding::Bound(binding)
                }
                _ => continue,
            };
            let target = selected_target.unwrap_or_default().to_owned();
            let plan_name = satisfies_plan_name(
                &target,
                schema.identity_text(schema.trait_name),
                &provider_type,
            );
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
                requirement_lifetime_partition: Vec::new(),
                binding,
            });
            plans[position]
                .provenance
                .row_requirements
                .push(requirement.symbol);
            plans[position]
                .provenance
                .row_realizations
                .push(machine.symbol);
            plans[position].provenance.row_receiver_scopes.push(None);
            plans[position]
                .provenance
                .row_target_machine_origins
                .push(None);
        }
    }
    plans
}

/// The exact source edge's boundary application, including its full outer
/// telescope. It never claims a descendant schema on behalf of its data owner.
fn provider_plan_schema_target(
    context: ProviderPlanningContext<'_>,
    calling_plans: &[crate::calling_policy_plans::BoundaryCallingPlanRealization],
    scope: checked_trees::typed_trees::trait_definition::AppliedTraitScope<'_>,
) -> Option<(ProviderSchemaDeclaration, String, ServiceSchema)> {
    let definition = scope.declaration;
    if !definition.is_boundary {
        return None;
    }
    let arguments = context
        .typed
        .trait_type_parameters(definition)
        .iter()
        .map(|parameter| scope.parameter_argument(context.typed, parameter.symbol))
        .collect::<Vec<_>>();
    let schema =
        crate::service_schema::from_typed_application(context, calling_plans, scope, &arguments)?;
    Some((
        ProviderSchemaDeclaration::BoundaryTrait(definition.symbol),
        schema.identity_text(schema.trait_name).to_owned(),
        schema,
    ))
}

fn derive_boundary_operator_plans(
    context: ProviderPlanningContext<'_>,
    selected_target: Option<&str>,
    evaluated_bindings: Option<&crate::evaluated_via_bindings::EvaluatedViaBindingTable>,
) -> Vec<DerivedProviderPlan> {
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
        for clause in typed.machine_trait_conformances(machine) {
            let Some(requirement) = clause.requirement.as_ref() else {
                continue;
            };
            let Some(operator) =
                checked_trees::typed_trees::operator::resolve_satisfied_boundary_operator_for_conformance(
                    typed, machine, clause,
                )
            else {
                continue;
            };
            // A token-bearing requirement has a signature view for overload
            // matching, not a second provider slot. Its machine declaration
            // owns the schema and provenance in derive_top_level_requirement_plans.
            if typed.machine(operator.symbol).is_some() {
                continue;
            }
            let binding = match (machine.supply_mode, clause.external_binding) {
                (
                    language_semantics::MachineSupplyMode::ExternalRealization { .. },
                    Some(conformance_binding),
                ) if !clause.via_expression.is_valid() => {
                    let Some(binding) = exact_installed_external_binding_identity(
                        typed,
                        machine,
                        conformance_binding,
                        clause.name.as_str(),
                        requirement.as_str(),
                    ) else {
                        continue;
                    };
                    DraftRowBinding::Bound(external_provider_binding(
                        binding,
                        &typed.attached_data_path(machine).unwrap_or_default(),
                        &typed
                            .machine_callable_identity_text(machine)
                            .unwrap_or_default(),
                    ))
                }
                (
                    language_semantics::MachineSupplyMode::ExternalRealization {
                        binding: None,
                        mechanism: None,
                    },
                    None,
                ) if !machine.body_is_present && clause.via_expression.is_valid() => {
                    let Some(binding) =
                        evaluated_via_binding(evaluated_bindings, machine, clause, selected_target)
                    else {
                        continue;
                    };
                    DraftRowBinding::Bound(binding)
                }
                (language_semantics::MachineSupplyMode::CheckedBody, None)
                    if machine.body_is_present && !clause.via_expression.is_valid() =>
                {
                    checked_adapter_draft(typed, machine)
                }
                _ => continue, // invalid via/body combinations are refused elsewhere
            };
            let Some(schema) = crate::service_schema::from_typed_operator(context, operator) else {
                continue;
            };
            let target = selected_target.unwrap_or_default().to_owned();
            let provider_type = typed.attached_data_path(machine).unwrap_or_default();
            let plan_name = satisfies_plan_name(
                &target,
                schema.identity_text(schema.trait_name),
                &provider_type,
            );
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
                schema.identity_text(schema.methods[0].requirement_identity),
            );
            plans[position].plan.rows.push(ProviderPlanRow {
                method: "realize".to_owned(),
                requirement_identity,
                requirement_lifetime_partition: Vec::new(),
                binding,
            });
            plans[position]
                .provenance
                .row_requirements
                .push(operator.symbol);
            plans[position]
                .provenance
                .row_realizations
                .push(machine.symbol);
            plans[position].provenance.row_receiver_scopes.push(None);
            plans[position]
                .provenance
                .row_target_machine_origins
                .push(None);
        }
    }
    plans
}

/// A provider row binding whose checked-adapter identity still needs to be
/// interned into its owning plan's table. Rows reach their plan after the
/// draft is built, so canonical identity text rides along until the push
/// site resolves it against `plan.identities`.
enum DraftRowBinding {
    /// A binding that carries no plan-local identities.
    Bound(ProviderBinding),
    /// The canonical machine identity text a checked-adapter binding interns
    /// into the owning plan's `identities`.
    CheckedAdapter {
        machine_identity: String,
        machine_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    },
}

impl DraftRowBinding {
    /// The row binding with any deferred identity interned into `identities`.
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

/// Specialization preserves satisfaction edges for checking the concrete body,
/// but does not author another provider declaration. Select the template once;
/// exact specialization receipts join its closed bodies after plan selection.
fn is_authored_provider_machine(
    typed: &checked_trees::typed_trees::TypedTrees,
    machine: &checked_trees::typed_trees::machine::Machine,
) -> bool {
    !typed
        .machine_specializations
        .iter()
        .any(|receipt| receipt.instance == machine.symbol)
}

/// The checked-adapter provider binding a machine supplies through its own
/// checked body: the machine's callable identity plus its package identity.
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

/// The provider binding an exact evaluated via-binding row supplies for a
/// machine's `via` conformance clause: the evaluated table's recorded
/// binding when the row exists.
fn evaluated_via_binding(
    evaluated_bindings: Option<&crate::evaluated_via_bindings::EvaluatedViaBindingTable>,
    machine: &checked_trees::typed_trees::machine::Machine,
    clause: &checked_trees::typed_trees::machine::TraitConformance,
    selected_target: Option<&str>,
) -> Option<ProviderBinding> {
    let table = evaluated_bindings?;
    let row = table.exact(machine.symbol, clause.symbol, clause.requirement_symbol)?;
    if selected_target != Some(row.evaluated().target().target_name())
        || table.target() != Some(row.evaluated().target())
    {
        return None;
    }
    Some(row.evaluated().provider_binding())
}

/// The position of the derived plan keyed by its name, provider and origin
/// package identities, schema declaration and provider symbol — inserting the
/// empty plan when absent.
#[allow(clippy::too_many_arguments)]
/// Mint one row's exact requirement identity into the plan's identity table.
/// The table is append-only, so rows pushed earlier keep resolving.
fn mint_plan_row_identity(
    plan: &mut ProviderPlan,
    requirement_identity: &str,
) -> ProviderIdentityId {
    plan.identities.intern(requirement_identity)
}

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
                && derived.plan.schema == schema
                && derived.provenance.provider_type == provider_type_symbol
        })
        .unwrap_or_else(|| {
            plans.push(DerivedProviderPlan {
                plan: ProviderPlan {
                    name: plan_name,
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
                    row_requirements: Vec::new(),
                    row_realizations: Vec::new(),
                    row_receiver_scopes: Vec::new(),
                    row_target_machine_origins: Vec::new(),
                },
            });
            plans.len() - 1
        })
}
