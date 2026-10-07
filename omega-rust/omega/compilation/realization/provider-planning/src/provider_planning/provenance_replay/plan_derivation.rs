//! Deriving satisfies plans, top-level requirement plans and boundary
//! operator plans with their provenance.
//!
//! Provider paths use the same complete nominal projection as Build selection
//! and Terminal candidate emission. They do not replace declaration custody:
//! grouping/provenance retain exact provider symbols, packages and normalized
//! callable identities, which independent replay must still rejoin.

use crate::ProviderPlanningContext;
use crate::provider_planning::operator_provider_evidence::{
    provider_type_package_identity, provider_type_symbol,
};
use crate::provider_planning::provenance_replay::requirement_identities::{
    exact_installed_external_binding_identity, exact_satisfied_requirement_identity,
    external_provider_binding, inferred_compiler_leaf_binding, realization_machine_identity,
};
use crate::provider_planning::provenance_replay::{
    DerivedProviderPlan, ProviderPlanProvenance, ProviderSchemaDeclaration,
    SelectedTargetMachineOrigin, satisfies_plan_name,
};
use crate::provider_planning::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema};

pub(crate) fn derive_provider_plans(
    context: ProviderPlanningContext<'_>,
    selected_target: Option<&str>,
    evaluated_bindings: Option<&crate::evaluated_via_bindings::EvaluatedViaBindingTable>,
    target_machine_origins: &[SelectedTargetMachineOrigin],
) -> Vec<DerivedProviderPlan> {
    let typed = context.typed;
    let mut plans: Vec<DerivedProviderPlan> = Vec::new();
    // Typed trees retain every authored body and its conformances. This
    // transitional settlement derives only the promoted realization's rows;
    // semantic declaration queries must never perform that selection for us.
    for machine in typed.machines() {
        if machine.target.is_some() || !context.contains(machine.symbol) {
            continue;
        }
        let origin_package_identity = typed.symbols.symbol_package_identity(machine.symbol);
        let provider_type_package_identity = provider_type_package_identity(typed, machine);
        let provider_type_symbol = provider_type_symbol(typed, machine);
        for clause in typed.machine_trait_conformances(machine) {
            if clause.requirement.is_some()
                && typed_trees::operator::resolve_satisfied_boundary_operator_for_conformance(
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
            let (row_binding, target_machine_origin) =
                match (machine.supply_mode, clause.external_binding) {
                    (language_semantics::MachineSupplyMode::Boundary, None)
                        if !machine.body_is_present
                            && !clause.via_expression.is_valid()
                            && clause.external_binding_source_span.is_none() =>
                    {
                        let Some((binding, origin)) = inferred_compiler_leaf_binding(
                            typed,
                            machine,
                            clause,
                            selected_target,
                            target_machine_origins,
                        ) else {
                            continue;
                        };
                        (binding, Some(origin))
                    }
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
                        let binding = external_provider_binding(
                            binding,
                            &typed.attached_data_path(machine).unwrap_or_default(),
                            &realization_machine_identity(typed, machine),
                        );
                        (binding, None)
                    }
                    (
                        language_semantics::MachineSupplyMode::ExternalRealization {
                            binding: None,
                            mechanism: None,
                        },
                        None,
                    ) if !machine.body_is_present && clause.via_expression.is_valid() => {
                        let Some(binding) = evaluated_via_binding(
                            evaluated_bindings,
                            machine,
                            clause,
                            selected_target,
                        ) else {
                            continue;
                        };
                        (binding, None)
                    }
                    (language_semantics::MachineSupplyMode::CheckedBody, None)
                        if machine.body_is_present && !clause.via_expression.is_valid() =>
                    {
                        // A CHECKED ADAPTER derives a plan row only over a
                        // BOUNDARY trait (a service schema). A plain trait's
                        // conformance -- including its service-reach ceiling -- is the
                        // existing trait machinery's business (the decision-20
                        // admission fixtures pin it) and derives nothing here.
                        let is_boundary_trait = typed.traits().iter().any(|definition| {
                            definition.is_boundary && definition.symbol == clause.symbol
                        });
                        if !is_boundary_trait {
                            continue;
                        }
                        (
                            ProviderBinding::CheckedAdapter {
                                machine_identity: typed
                                    .machine_callable_identity_text(machine)
                                    .unwrap_or_default(),
                                machine_package_identity: typed
                                    .symbols
                                    .symbol_package_identity(machine.symbol),
                            },
                            None,
                        )
                    }
                    _ => continue, // refused elsewhere (via rungs)
                };
            let target = selected_target.unwrap_or_default().to_owned();
            let provider_type = typed.attached_data_path(machine).unwrap_or_default();
            let semantic_requirement_identity = exact_satisfied_requirement_identity(
                typed,
                clause.symbol,
                clause.requirement_symbol,
            );
            let requirement_symbol = clause.requirement_symbol;
            if let Some((schema_declaration, schema_trait, schema)) =
                provider_plan_schema_target(context, clause.symbol)
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
                plans[position].plan.rows.push(ProviderPlanRow {
                    method: requirement.as_str().to_owned(),
                    requirement_identity: semantic_requirement_identity.clone(),
                    requirement_lifetime_partition:
                        typed_trees::machine::normalize_requirement_lifetime_partition(
                            &clause.trait_lifetime_arguments,
                        ),
                    binding: row_binding.clone(),
                });
                plans[position]
                    .provenance
                    .row_requirements
                    .push(requirement_symbol);
                plans[position]
                    .provenance
                    .row_realizations
                    .push(machine.symbol);
                plans[position]
                    .provenance
                    .row_target_machine_origins
                    .push(target_machine_origin.clone());
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
    let named = derive_named_provider_plans(context, selected_target, &plans);
    plans.extend(named);
    plans
}

fn derive_named_provider_plans(
    context: ProviderPlanningContext<'_>,
    selected_target: Option<&str>,
    candidates: &[DerivedProviderPlan],
) -> Vec<DerivedProviderPlan> {
    let typed = context.typed;
    typed
        .conformances()
        .iter()
        .filter_map(|implementation| {
            if !crate::provider_identity::eligible(implementation)
                || !context.contains(implementation.symbol)
            {
                return None;
            }
            let rows = &implementation.rows;
            let definition = typed.traits().iter().find(|definition| {
                definition.symbol == implementation.trait_symbol
                    && definition.is_boundary
                    && context.contains(definition.symbol)
            })?;
            let arguments = typed
                .type_reference_table
                .type_reference_handles(implementation.arguments);
            let schema =
                crate::service_schema::from_typed_instance(context, definition, arguments)?;
            let provider_type = typed.symbols.display_path(implementation.symbol, "::");
            let package = typed.symbols.symbol_package_identity(implementation.symbol);
            let origin_package = typed
                .symbols
                .symbol_package_identity(rows.first()?.realization_machine);
            let target = selected_target.unwrap_or_default().to_owned();
            let mut plan = ProviderPlan {
                name: satisfies_plan_name(&target, &schema.trait_name, &provider_type),
                provider_type,
                provider_type_package_identity: package,
                target,
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
                row_target_machine_origins: Vec::new(),
            };
            for row in rows {
                let machine = typed.machines().iter().find(|machine| {
                    machine.symbol == row.realization_machine
                        && machine.target.is_none()
                        && context.contains(machine.symbol)
                        && typed.symbols.symbol_package_identity(machine.symbol) == origin_package
                })?;
                if typed.machine_states(machine).first()?.symbol != row.realization_state {
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
                            machine_identity: typed.machine_callable_identity_text(machine)?,
                            machine_package_identity: typed
                                .symbols
                                .symbol_package_identity(machine.symbol),
                        },
                        typed_trees::machine::normalize_requirement_lifetime_partition(
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
                                    == Some(&row.realization_machine)
                                && binding.requirement_identity == identity
                            {
                                matched.push((
                                    binding,
                                    candidate.provenance.row_target_machine_origins.get(index)?,
                                ));
                            }
                        }
                    }
                    let (binding, origin) = *matched.first()?;
                    if matched
                        .iter()
                        .any(|(other, other_origin)| *other != binding || *other_origin != origin)
                    {
                        return None;
                    }
                    let origin = origin.clone();
                    (
                        binding.binding.clone(),
                        binding.requirement_lifetime_partition.clone(),
                        origin,
                    )
                };
                plan.rows.push(ProviderPlanRow {
                    method: row.requirement_name.as_str().to_owned(),
                    requirement_identity: identity,
                    requirement_lifetime_partition: lifetime_partition,
                    binding,
                });
                provenance.row_requirements.push(row.requirement);
                provenance.row_realizations.push(machine.symbol);
                provenance.row_target_machine_origins.push(origin);
            }
            Some(DerivedProviderPlan { plan, provenance })
        })
        .collect()
}

fn derive_top_level_requirement_plans(
    context: ProviderPlanningContext<'_>,
    selected_target: Option<&str>,
    evaluated_bindings: Option<&crate::evaluated_via_bindings::EvaluatedViaBindingTable>,
) -> Vec<DerivedProviderPlan> {
    let typed = context.typed;
    let mut plans = Vec::<DerivedProviderPlan>::new();
    for machine in typed.machines() {
        if machine.target.is_some() || !context.contains(machine.symbol) {
            continue;
        }
        // A specialization instance is minted from its authored template,
        // not authored as a provider: the generic provider template derives
        // the selected plan, and the selected-requirement specialization
        // roster joins each closed requirement instance to it.
        if typed
            .machine_specializations
            .iter()
            .any(|receipt| receipt.instance == machine.symbol)
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
                || schema_method.requirement_identity != requirement_identity
            {
                continue;
            }
            let binding = match (machine.supply_mode, clause.external_binding) {
                (language_semantics::MachineSupplyMode::CheckedBody, None)
                    if machine.body_is_present && !clause.via_expression.is_valid() =>
                {
                    checked_adapter_binding(typed, machine)
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
                    external_provider_binding(
                        binding,
                        &provider_type,
                        &realization_machine_identity(typed, machine),
                    )
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
                    binding
                }
                _ => continue,
            };
            let target = selected_target.unwrap_or_default().to_owned();
            let plan_name = satisfies_plan_name(&target, &schema.trait_name, &provider_type);
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
            plans[position]
                .provenance
                .row_target_machine_origins
                .push(None);
        }
    }
    plans
}

/// The boundary schema under which an exact requirement edge is installed:
/// the edge's own boundary trait, with no whole-trait application. A root that
/// inherits the requirement and adds `Calling<C>` is installed through a named
/// implementation of that root, whose plan `derive_named_provider_plans`
/// derives from its explicit rows; an exact edge never claims a descendant
/// schema on behalf of the data that owns its machine.
fn provider_plan_schema_target(
    context: ProviderPlanningContext<'_>,
    satisfied_trait_symbol: symbols::SymbolHandle,
) -> Option<(ProviderSchemaDeclaration, String, ServiceSchema)> {
    let definition =
        context.typed.traits().iter().find(|definition| {
            definition.is_boundary && definition.symbol == satisfied_trait_symbol
        })?;
    let schema = crate::service_schema::from_typed_instance(context, definition, &[])?;
    Some((
        ProviderSchemaDeclaration::BoundaryTrait(definition.symbol),
        schema.trait_name.clone(),
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
        if machine.target.is_some() || !context.contains(machine.symbol) {
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
                typed_trees::operator::resolve_satisfied_boundary_operator_for_conformance(
                    typed, machine, clause,
                )
            else {
                continue;
            };
            // A token-bearing requirement has a signature view for overload
            // matching, not a second provider slot. Its machine declaration
            // owns the schema and provenance in derive_top_level_requirement_plans.
            if typed
                .machines()
                .iter()
                .any(|requirement| requirement.symbol == operator.symbol)
            {
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
                    external_provider_binding(
                        binding,
                        &typed.attached_data_path(machine).unwrap_or_default(),
                        &typed
                            .machine_callable_identity_text(machine)
                            .unwrap_or_default(),
                    )
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
                    binding
                }
                (language_semantics::MachineSupplyMode::CheckedBody, None)
                    if machine.body_is_present && !clause.via_expression.is_valid() =>
                {
                    checked_adapter_binding(typed, machine)
                }
                _ => continue, // invalid via/body combinations are refused elsewhere
            };
            let Some(schema) = crate::service_schema::from_typed_operator(context, operator) else {
                continue;
            };
            let target = selected_target.unwrap_or_default().to_owned();
            let provider_type = typed.attached_data_path(machine).unwrap_or_default();
            let plan_name = satisfies_plan_name(&target, &schema.trait_name, &provider_type);
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
            plans[position].plan.rows.push(ProviderPlanRow {
                method: "realize".to_owned(),
                requirement_identity: schema.methods[0].requirement_identity.clone(),
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
            plans[position]
                .provenance
                .row_target_machine_origins
                .push(None);
        }
    }
    plans
}

/// The checked-adapter provider binding a machine supplies through its own
/// checked body: the machine's callable identity plus its package identity.
fn checked_adapter_binding(
    typed: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> ProviderBinding {
    ProviderBinding::CheckedAdapter {
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
    machine: &typed_trees::machine::Machine,
    clause: &typed_trees::machine::TraitConformance,
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
                    provider_type,
                    provider_type_package_identity,
                    target,
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
                    row_target_machine_origins: Vec::new(),
                },
            });
            plans.len() - 1
        })
}
