//! Deriving satisfies plans, top-level requirement plans and boundary
//! operator plans with their provenance.
//!
//! Provider spelling is the existing Terminal catalog projection, not a
//! substitute for declaration custody. Keep the exact provider symbol in
//! grouping/provenance and the normalized callable identity on every row.
//! Migrating the spelling requires changing Terminal candidate emission and
//! selection together; changing this producer alone breaks installation.

use crate::provider_planning::operator_provider_evidence::{
    provider_type_package_identity, provider_type_symbol,
};
use crate::provider_planning::provenance_replay::requirement_identities::{
    exact_installed_external_binding_identity, exact_satisfied_requirement_identity,
    external_provider_binding, inferred_hosted_console_compiler_intrinsic,
    provider_boundary_arguments, realization_machine_identity,
};
use crate::provider_planning::provenance_replay::{
    DerivedProviderPlan, ProviderPlanProvenance, ProviderSchemaDeclaration,
    SelectedTargetMachineOrigin, satisfies_plan_name,
};
use crate::provider_planning::{
    ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema, TypedTrees,
};

pub(crate) fn derive_provider_plans(
    typed: &TypedTrees,
    selected_target: Option<&str>,
    evaluated_bindings: Option<&crate::evaluated_via_bindings::EvaluatedViaBindingTable>,
    target_machine_origins: &[SelectedTargetMachineOrigin],
) -> Vec<DerivedProviderPlan> {
    let mut plans: Vec<DerivedProviderPlan> = Vec::new();
    // Target filtering has already admitted only unscoped and selected-target
    // machines into typed trees. Derive from their exact retained conformance
    // and supply identities; source syntax is no longer a binding authority.
    for machine in typed.machines() {
        if !crate::service_schema::is_product_declaration(typed, machine.symbol) {
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
                        let Some((binding, origin)) = inferred_hosted_console_compiler_intrinsic(
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
                            machine
                                .attached_data
                                .as_ref()
                                .map(|name| name.as_str())
                                .unwrap_or_default(),
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
                        let Some(row) = evaluated_bindings.and_then(|table| {
                            table.exact(machine.symbol, clause.symbol, clause.requirement_symbol)
                        }) else {
                            continue;
                        };
                        (row.evaluated().provider_binding(), None)
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
                                    .normalized_machine_overload_identity(machine)
                                    .map(|identity| identity.identity())
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
            let provider_type = machine
                .attached_data
                .as_ref()
                .map(|name| name.as_str().to_owned())
                .unwrap_or_default();
            let semantic_requirement_identity = exact_satisfied_requirement_identity(
                typed,
                clause.symbol,
                clause.requirement_symbol,
            );
            let requirement_symbol = clause.requirement_symbol;
            for (schema_declaration, schema_trait, schema) in provider_plan_schema_targets(
                typed,
                provider_type_symbol,
                clause.symbol,
                &semantic_requirement_identity,
            ) {
                let plan_name = satisfies_plan_name(&target, &schema_trait, &provider_type);
                let position = plans
                    .iter()
                    .position(|derived| {
                        derived.plan.name == plan_name
                            && derived.plan.provider_type_package_identity
                                == provider_type_package_identity
                            && derived.plan.origin_package_identity == origin_package_identity
                            && derived.provenance.schema == schema_declaration
                            && derived.provenance.provider_type == provider_type_symbol
                    })
                    .unwrap_or_else(|| {
                        plans.push(DerivedProviderPlan {
                            plan: ProviderPlan {
                                name: plan_name.clone(),
                                provider_type: provider_type.clone(),
                                provider_type_package_identity,
                                target: target.clone(),
                                schema,
                                rows: Vec::new(),
                                origin_package_identity,
                                origin_package: String::new(),
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
                    });
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
        typed,
        selected_target,
        evaluated_bindings,
    ));
    plans.extend(derive_top_level_requirement_plans(
        typed,
        selected_target,
        evaluated_bindings,
    ));
    plans
}

fn derive_top_level_requirement_plans(
    typed: &TypedTrees,
    selected_target: Option<&str>,
    evaluated_bindings: Option<&crate::evaluated_via_bindings::EvaluatedViaBindingTable>,
) -> Vec<DerivedProviderPlan> {
    let mut plans = Vec::<DerivedProviderPlan>::new();
    for machine in typed.machines() {
        if !crate::service_schema::is_product_declaration(typed, machine.symbol) {
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
        let Some(provider_type) = machine
            .attached_data
            .as_ref()
            .map(|name| name.as_str().to_owned())
        else {
            continue;
        };
        let provider_type_package_identity = provider_type_package_identity(typed, machine);
        let origin_package_identity = typed.symbols.symbol_package_identity(machine.symbol);
        for clause in typed.machine_trait_conformances(machine) {
            let Some(typed_trees::machine::SatisfiedDeclaration::TopLevelRequirement(requirement)) =
                typed_trees::machine::resolve_satisfied_declaration(typed, machine, clause)
            else {
                continue;
            };
            if !requirement.is_public
                || clause.symbol != requirement.symbol
                || clause.requirement_symbol != requirement.symbol
            {
                continue;
            }
            let Some(schema) =
                crate::service_schema::from_typed_boundary_requirement(typed, requirement)
            else {
                continue;
            };
            let requirement_identity = typed
                .normalized_machine_overload_identity(requirement)
                .map(|identity| identity.identity())
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
                    ProviderBinding::CheckedAdapter {
                        machine_identity: typed
                            .normalized_machine_overload_identity(machine)
                            .map(|identity| identity.identity())
                            .unwrap_or_default(),
                        machine_package_identity: typed
                            .symbols
                            .symbol_package_identity(machine.symbol),
                    }
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
                    let Some(row) = evaluated_bindings.and_then(|table| {
                        table.exact(machine.symbol, clause.symbol, clause.requirement_symbol)
                    }) else {
                        continue;
                    };
                    row.evaluated().provider_binding()
                }
                _ => continue,
            };
            let target = selected_target.unwrap_or_default().to_owned();
            let plan_name = satisfies_plan_name(&target, &schema.trait_name, &provider_type);
            let position = plans
                .iter()
                .position(|derived| {
                    derived.plan.name == plan_name
                        && derived.plan.provider_type_package_identity
                            == provider_type_package_identity
                        && derived.plan.origin_package_identity == origin_package_identity
                        && derived.provenance.schema
                            == ProviderSchemaDeclaration::BoundaryRequirement(requirement.symbol)
                        && derived.provenance.provider_type == Some(provider_type_symbol)
                })
                .unwrap_or_else(|| {
                    plans.push(DerivedProviderPlan {
                        plan: ProviderPlan {
                            name: plan_name.clone(),
                            provider_type: provider_type.clone(),
                            provider_type_package_identity,
                            target: target.clone(),
                            schema: schema.clone(),
                            rows: Vec::new(),
                            origin_package_identity,
                            origin_package: String::new(),
                        },
                        provenance: ProviderPlanProvenance {
                            schema: ProviderSchemaDeclaration::BoundaryRequirement(
                                requirement.symbol,
                            ),
                            provider_type: Some(provider_type_symbol),
                            row_requirements: Vec::new(),
                            row_realizations: Vec::new(),
                            row_target_machine_origins: Vec::new(),
                        },
                    });
                    plans.len() - 1
                });
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

/// Select the boundary schema under which an exact inherited routed-input
/// requirement is installed. A provider may implement the stable parent
/// requirement while explicitly conforming to a target root that inherits it
/// and adds `Calling<C>`. In that case the descendant schema owns plan/ABI
/// refinement, but the row keeps the parent's exact requirement identity.
/// A checked calling application also selects the descendant schema without
/// requiring an unrelated routed entry claim. Other requirements retain their
/// direct provider-plan behavior.
fn provider_plan_schema_targets(
    typed: &TypedTrees,
    provider_type_symbol: Option<symbols::SymbolHandle>,
    satisfied_trait_symbol: symbols::SymbolHandle,
    requirement_identity: &str,
) -> Vec<(ProviderSchemaDeclaration, String, ServiceSchema)> {
    let direct = typed
        .traits()
        .iter()
        .find(|definition| definition.is_boundary && definition.symbol == satisfied_trait_symbol);

    let mut refined = typed
        .conformances()
        .iter()
        .filter(|conformance| Some(conformance.carrier_symbol) == provider_type_symbol)
        .filter_map(|conformance| {
            let definition = typed.traits().iter().find(|definition| {
                definition.is_boundary && definition.symbol == conformance.trait_symbol
            })?;
            let arguments = provider_boundary_arguments(typed, definition, provider_type_symbol);
            let schema = crate::service_schema::from_typed_instance(typed, definition, &arguments)?;
            schema
                .methods
                .iter()
                .any(|method| {
                    method.requirement_identity == requirement_identity
                        && (!method.entry_claims.is_empty()
                            || method.calling_plan_commitment.is_some())
                })
                .then(|| {
                    (
                        ProviderSchemaDeclaration::BoundaryTrait(definition.symbol),
                        schema.trait_name.clone(),
                        schema,
                    )
                })
        })
        .collect::<Vec<_>>();
    refined.sort_by(|left, right| left.1.cmp(&right.1));
    refined.dedup_by(|left, right| left.0 == right.0);

    let has_descendant = refined.iter().any(|(schema, _, _)| {
        direct.is_some_and(|definition| schema.symbol() != definition.symbol)
    });
    if has_descendant {
        refined.retain(|(schema, _, _)| {
            direct.is_none_or(|definition| schema.symbol() != definition.symbol)
        });
    }
    if !refined.is_empty() {
        return refined;
    }

    direct
        .and_then(|definition| {
            let arguments = provider_boundary_arguments(typed, definition, provider_type_symbol);
            crate::service_schema::from_typed_instance(typed, definition, &arguments).map(
                |schema| {
                    (
                        ProviderSchemaDeclaration::BoundaryTrait(definition.symbol),
                        schema.trait_name.clone(),
                        schema,
                    )
                },
            )
        })
        .into_iter()
        .collect()
}

fn derive_boundary_operator_plans(
    typed: &TypedTrees,
    selected_target: Option<&str>,
    evaluated_bindings: Option<&crate::evaluated_via_bindings::EvaluatedViaBindingTable>,
) -> Vec<DerivedProviderPlan> {
    let mut plans = Vec::<DerivedProviderPlan>::new();
    for machine in typed.machines() {
        if !crate::service_schema::is_product_declaration(typed, machine.symbol) {
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
                        machine
                            .attached_data
                            .as_ref()
                            .map(|name| name.as_str())
                            .unwrap_or_default(),
                        &typed
                            .normalized_machine_overload_identity(machine)
                            .map(|identity| identity.identity())
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
                    let Some(row) = evaluated_bindings.and_then(|table| {
                        table.exact(machine.symbol, clause.symbol, clause.requirement_symbol)
                    }) else {
                        continue;
                    };
                    row.evaluated().provider_binding()
                }
                (language_semantics::MachineSupplyMode::CheckedBody, None)
                    if machine.body_is_present && !clause.via_expression.is_valid() =>
                {
                    ProviderBinding::CheckedAdapter {
                        machine_identity: typed
                            .normalized_machine_overload_identity(machine)
                            .map(|identity| identity.identity())
                            .unwrap_or_default(),
                        machine_package_identity: typed
                            .symbols
                            .symbol_package_identity(machine.symbol),
                    }
                }
                _ => continue, // invalid via/body combinations are refused elsewhere
            };
            let Some(schema) = crate::service_schema::from_typed_operator(typed, operator) else {
                continue;
            };
            let target = selected_target.unwrap_or_default().to_owned();
            let provider_type = machine
                .attached_data
                .as_ref()
                .map(|name| name.as_str().to_owned())
                .unwrap_or_default();
            let plan_name = satisfies_plan_name(&target, &schema.trait_name, &provider_type);
            let position = plans
                .iter()
                .position(|derived| {
                    derived.plan.name == plan_name
                        && derived.plan.provider_type_package_identity
                            == provider_type_package_identity
                        && derived.plan.origin_package_identity == origin_package_identity
                        && derived.provenance.schema
                            == ProviderSchemaDeclaration::BoundaryOperator(operator.symbol)
                        && derived.provenance.provider_type == provider_type_symbol
                })
                .unwrap_or_else(|| {
                    plans.push(DerivedProviderPlan {
                        plan: ProviderPlan {
                            name: plan_name.clone(),
                            provider_type: provider_type.clone(),
                            provider_type_package_identity,
                            target: target.clone(),
                            schema: schema.clone(),
                            rows: Vec::new(),
                            origin_package_identity,
                            origin_package: String::new(),
                        },
                        provenance: ProviderPlanProvenance {
                            schema: ProviderSchemaDeclaration::BoundaryOperator(operator.symbol),
                            provider_type: provider_type_symbol,
                            row_requirements: Vec::new(),
                            row_realizations: Vec::new(),
                            row_target_machine_origins: Vec::new(),
                        },
                    });
                    plans.len() - 1
                });
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
