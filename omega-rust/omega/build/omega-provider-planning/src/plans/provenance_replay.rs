//! Exact replay of derived and selected provider provenance.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderSchemaDeclaration {
    BoundaryTrait(psi_symbols::SymbolHandle),
    BoundaryRequirement(psi_symbols::SymbolHandle),
    BoundaryOperator(psi_symbols::SymbolHandle),
}

impl ProviderSchemaDeclaration {
    pub const fn symbol(self) -> psi_symbols::SymbolHandle {
        match self {
            Self::BoundaryTrait(symbol)
            | Self::BoundaryRequirement(symbol)
            | Self::BoundaryOperator(symbol) => symbol,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPlanProvenance {
    pub schema: ProviderSchemaDeclaration,
    pub provider_type: Option<psi_symbols::SymbolHandle>,
    pub row_requirements: Vec<psi_symbols::SymbolHandle>,
    pub row_realizations: Vec<psi_symbols::SymbolHandle>,
    /// Exact pre-resolution target-scoped declaration custody for catalog-
    /// inferred rows. `None` is mandatory for ordinary checked, legacy
    /// external, and evaluated-payload rows.
    pub row_target_machine_origins: Vec<Option<SelectedTargetMachineOrigin>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedTargetMachineOrigin {
    pub machine: psi_symbols::SymbolHandle,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedProviderPlan {
    pub plan: ProviderPlan,
    pub provenance: ProviderPlanProvenance,
}

pub fn derive_satisfies_plans_with_provenance(
    typed: &TypedTrees,
    selected_target: Option<&str>,
) -> Vec<DerivedProviderPlan> {
    derive_satisfies_plans_with_optional_evaluated_bindings(typed, selected_target, None, &[])
}

/// Strict production derivation after all ordinary `via` expressions have
/// been evaluated. Legacy external leaves remain on their segregated carrier;
/// an ordinary `via` row can only consume its exact table entry.
pub fn derive_satisfies_plans_with_evaluated_bindings(
    typed: &TypedTrees,
    selected_target: Option<&str>,
    evaluated_bindings: &crate::evaluated_via_bindings::EvaluatedViaBindingTable,
) -> Result<Vec<DerivedProviderPlan>, Vec<psi_diagnostics::Diagnostic>> {
    evaluated_bindings.validate_against_typed(typed)?;
    let retained_target = evaluated_bindings
        .target()
        .map(omega_target::TargetProfile::target_name);
    if retained_target != selected_target {
        return Err(vec![psi_diagnostics::Diagnostic::error(format!(
            "evaluated `via` binding table target `{}` does not match provider derivation target `{}`",
            retained_target.unwrap_or("<none>"),
            selected_target.unwrap_or("<none>"),
        ))]);
    }
    Ok(derive_satisfies_plans_with_optional_evaluated_bindings(
        typed,
        selected_target,
        Some(evaluated_bindings),
        &[],
    ))
}

pub fn derive_satisfies_plans_with_evaluated_bindings_and_target_machine_origins(
    typed: &TypedTrees,
    selected_target: Option<&str>,
    evaluated_bindings: &crate::evaluated_via_bindings::EvaluatedViaBindingTable,
    target_machine_origins: &[SelectedTargetMachineOrigin],
) -> Result<Vec<DerivedProviderPlan>, Vec<psi_diagnostics::Diagnostic>> {
    evaluated_bindings.validate_against_typed(typed)?;
    let retained_target = evaluated_bindings
        .target()
        .map(omega_target::TargetProfile::target_name);
    if retained_target != selected_target {
        return Err(vec![psi_diagnostics::Diagnostic::error(format!(
            "evaluated `via` binding table target `{}` does not match provider derivation target `{}`",
            retained_target.unwrap_or("<none>"),
            selected_target.unwrap_or("<none>"),
        ))]);
    }
    Ok(derive_satisfies_plans_with_optional_evaluated_bindings(
        typed,
        selected_target,
        Some(evaluated_bindings),
        target_machine_origins,
    ))
}

fn derive_satisfies_plans_with_optional_evaluated_bindings(
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
        let origin_package_identity = typed.symbols.symbol_package_identity(machine.symbol);
        let provider_type_package_identity = provider_type_package_identity(typed, machine);
        let provider_type_symbol = provider_type_symbol(typed, machine);
        for clause in typed.machine_trait_conformances(machine) {
            if clause.requirement.as_ref().is_some_and(|requirement| {
                psi_typed_trees::operator::resolve_satisfied_boundary_operator(
                    typed,
                    machine,
                    clause.name.as_str(),
                    requirement.as_str(),
                )
                .is_some()
            }) {
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
                    (psi_language_semantics::MachineSupplyMode::Boundary, None)
                        if !machine.body_is_present
                            && !clause.via_expression.is_valid()
                            && clause.external_binding_source_span.is_none() =>
                    {
                        let Some((binding, origin)) = inferred_linux_console_compiler_intrinsic(
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
                        psi_language_semantics::MachineSupplyMode::ExternalRealization { .. },
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
                        (
                            external_provider_binding(
                                binding,
                                machine
                                    .attached_data
                                    .as_ref()
                                    .map(|name| name.as_str())
                                    .unwrap_or_default(),
                                &realization_machine_identity(typed, machine.name.as_str()),
                            ),
                            None,
                        )
                    }
                    (
                        psi_language_semantics::MachineSupplyMode::ExternalRealization {
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
                    (psi_language_semantics::MachineSupplyMode::CheckedBody, None)
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
            let requirement_identity = satisfied_requirement_identity(
                typed,
                machine.name.as_str(),
                clause.name.as_str(),
                requirement.as_str(),
            );
            let semantic_requirement_identity = exact_satisfied_requirement_identity(
                typed,
                clause.symbol,
                clause.requirement_symbol,
            );
            let requirement_symbol = clause.requirement_symbol;
            for (schema_declaration, schema_trait, schema) in provider_plan_schema_targets(
                typed,
                &provider_type,
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
                    requirement_identity: requirement_identity.clone(),
                    requirement_lifetime_partition:
                        psi_typed_trees::machine::normalize_requirement_lifetime_partition(
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
    plans.extend(derive_boundary_operator_plans_with_provenance(
        typed,
        selected_target,
        evaluated_bindings,
    ));
    plans.extend(derive_top_level_requirement_plans_with_provenance(
        typed,
        selected_target,
        evaluated_bindings,
    ));
    plans
}

fn derive_top_level_requirement_plans_with_provenance(
    typed: &TypedTrees,
    selected_target: Option<&str>,
    evaluated_bindings: Option<&crate::evaluated_via_bindings::EvaluatedViaBindingTable>,
) -> Vec<DerivedProviderPlan> {
    let mut plans = Vec::<DerivedProviderPlan>::new();
    for machine in typed.machines() {
        let is_checked_adapter = machine.supply_mode
            == psi_language_semantics::MachineSupplyMode::CheckedBody
            && machine.body_is_present;
        let is_external_leaf = matches!(
            machine.supply_mode,
            psi_language_semantics::MachineSupplyMode::ExternalRealization { .. }
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
            let Some(psi_typed_trees::machine::SatisfiedDeclaration::TopLevelRequirement(
                requirement,
            )) = psi_typed_trees::machine::resolve_satisfied_declaration(typed, machine, clause)
            else {
                continue;
            };
            if !requirement.is_public
                || clause.symbol != requirement.symbol
                || clause.requirement_symbol != requirement.symbol
            {
                continue;
            }
            let Some(schema) = ServiceSchema::from_typed_boundary_requirement(typed, requirement)
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
                (psi_language_semantics::MachineSupplyMode::CheckedBody, None)
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
                    psi_language_semantics::MachineSupplyMode::ExternalRealization { .. },
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
                        &realization_machine_identity(typed, machine.name.as_str()),
                    )
                }
                (
                    psi_language_semantics::MachineSupplyMode::ExternalRealization {
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
/// Requirements without accepted entry claims retain the established direct
/// provider-plan behavior.
fn provider_plan_schema_targets(
    typed: &TypedTrees,
    provider_type: &str,
    provider_type_symbol: Option<psi_symbols::SymbolHandle>,
    satisfied_trait_symbol: psi_symbols::SymbolHandle,
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
            let arguments = provider_boundary_arguments(typed, definition, provider_type);
            let schema = ServiceSchema::from_typed_instance(typed, definition, &arguments)?;
            schema
                .methods
                .iter()
                .any(|method| {
                    method.requirement_identity == requirement_identity
                        && !method.entry_claims.is_empty()
                })
                .then(|| {
                    (
                        ProviderSchemaDeclaration::BoundaryTrait(definition.symbol),
                        definition.name.as_str().to_owned(),
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
            let arguments = provider_boundary_arguments(typed, definition, provider_type);
            ServiceSchema::from_typed_instance(typed, definition, &arguments).map(|schema| {
                (
                    ProviderSchemaDeclaration::BoundaryTrait(definition.symbol),
                    definition.name.as_str().to_owned(),
                    schema,
                )
            })
        })
        .into_iter()
        .collect()
}

fn derive_boundary_operator_plans_with_provenance(
    typed: &TypedTrees,
    selected_target: Option<&str>,
    evaluated_bindings: Option<&crate::evaluated_via_bindings::EvaluatedViaBindingTable>,
) -> Vec<DerivedProviderPlan> {
    let mut plans = Vec::<DerivedProviderPlan>::new();
    for machine in typed.machines() {
        let origin_package_identity = typed.symbols.symbol_package_identity(machine.symbol);
        let provider_type_package_identity = provider_type_package_identity(typed, machine);
        let provider_type_symbol = provider_type_symbol(typed, machine);
        for clause in typed.machine_trait_conformances(machine) {
            let Some(requirement) = clause.requirement.as_ref() else {
                continue;
            };
            let Some(operator) = psi_typed_trees::operator::resolve_satisfied_boundary_operator(
                typed,
                machine,
                clause.name.as_str(),
                requirement.as_str(),
            ) else {
                continue;
            };
            let binding = match (machine.supply_mode, clause.external_binding) {
                (
                    psi_language_semantics::MachineSupplyMode::ExternalRealization { .. },
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
                    psi_language_semantics::MachineSupplyMode::ExternalRealization {
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
                (psi_language_semantics::MachineSupplyMode::CheckedBody, None)
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
            let Some(schema) = ServiceSchema::from_typed_operator(typed, operator) else {
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

pub fn satisfied_requirement_identity(
    typed: &TypedTrees,
    machine_name: &str,
    trait_name: &str,
    requirement_name: &str,
) -> String {
    let Some(machine) = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
    else {
        return String::new();
    };
    if let Some(identity) = typed
        .machine_trait_conformances(machine)
        .iter()
        .filter(|conformance| {
            conformance.name.as_str() == trait_name
                && conformance.requirement.as_ref().map(|name| name.as_str())
                    == Some(requirement_name)
        })
        .find_map(|conformance| {
            let psi_typed_trees::machine::SatisfiedDeclaration::TopLevelRequirement(requirement) =
                psi_typed_trees::machine::resolve_satisfied_declaration(
                    typed,
                    machine,
                    conformance,
                )?
            else {
                return None;
            };
            typed
                .normalized_machine_overload_identity(requirement)
                .map(|identity| identity.identity())
        })
    {
        return identity;
    }
    let Some(definition) = typed.traits().iter().find(|definition| {
        definition.name.as_str() == trait_name
            || definition
                .name
                .as_str()
                .rsplit("::")
                .next()
                .is_some_and(|leaf| leaf == trait_name)
    }) else {
        return String::new();
    };
    let named = typed
        .trait_machine_signatures(definition)
        .iter()
        .filter(|signature| signature.name.as_str() == requirement_name)
        .collect::<Vec<_>>();
    let selected = match named.as_slice() {
        [single] => Some(*single),
        many => {
            let implementation_dispatch = typed
                .machine_states(machine)
                .first()
                .map(|entry| typed.normalized_result_dispatch_set(entry.return_type));
            let mut matching = many.iter().copied().filter(|signature| {
                implementation_dispatch.as_ref().is_some_and(|dispatch| {
                    typed.normalized_result_dispatch_set(signature.return_type) == *dispatch
                })
            });
            let selected = matching.next();
            selected.filter(|_| matching.next().is_none())
        }
    };
    selected
        .map(|signature| {
            typed
                .normalized_trait_requirement_overload_identity(definition, signature)
                .identity()
        })
        .unwrap_or_default()
}

fn exact_satisfied_requirement_identity(
    typed: &TypedTrees,
    trait_symbol: psi_symbols::SymbolHandle,
    requirement_symbol: psi_symbols::SymbolHandle,
) -> String {
    if trait_symbol == requirement_symbol
        && let Some(requirement) = typed.machines().iter().find(|requirement| {
            requirement.symbol == requirement_symbol
                && requirement.supply_mode
                    == psi_language_semantics::MachineSupplyMode::TopLevelRequirement
        })
    {
        return typed
            .normalized_machine_overload_identity(requirement)
            .map(|identity| identity.identity())
            .unwrap_or_default();
    }
    let Some(definition) = typed
        .traits()
        .iter()
        .find(|definition| definition.symbol == trait_symbol)
    else {
        return String::new();
    };
    typed
        .trait_machine_signatures(definition)
        .iter()
        .find(|signature| signature.symbol == requirement_symbol)
        .map(|signature| {
            typed
                .normalized_trait_requirement_overload_identity(definition, signature)
                .identity()
        })
        .unwrap_or_default()
}

fn exact_external_binding_identity<'typed>(
    typed: &'typed TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    trait_name: &str,
    requirement_name: &str,
) -> Option<&'typed psi_language_semantics::ExternalBindingIdentity> {
    let mut matching = typed
        .machine_trait_conformances(machine)
        .iter()
        .filter(|conformance| same_semantic_name(conformance.name.as_str(), trait_name))
        .filter(|conformance| {
            conformance.requirement.as_ref().map(|name| name.as_str()) == Some(requirement_name)
        })
        .filter_map(|conformance| conformance.external_binding);
    let binding = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    typed.external_bindings.identity(binding)
}

fn exact_installed_external_binding_identity<'typed>(
    typed: &'typed TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    conformance_binding: psi_language_semantics::ExternalBindingId,
    trait_name: &str,
    requirement_name: &str,
) -> Option<&'typed psi_language_semantics::ExternalBindingIdentity> {
    let psi_language_semantics::MachineSupplyMode::ExternalRealization {
        binding: Some(supply_binding),
        mechanism: Some(supply_mechanism),
    } = machine.supply_mode
    else {
        return None;
    };
    if supply_binding != conformance_binding {
        return None;
    }
    let binding = exact_external_binding_identity(typed, machine, trait_name, requirement_name)?;
    (binding.mechanism() == supply_mechanism).then_some(binding)
}

/// The first payload-free source-inferred compiler catalog leaf. This is only
/// candidate derivation: selected-dispatch independently rejoins package
/// custody and the canonical target before granting a closed execution.
fn inferred_linux_console_compiler_intrinsic(
    typed: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    conformance: &psi_typed_trees::machine::TraitConformance,
    selected_target: Option<&str>,
    target_machine_origins: &[SelectedTargetMachineOrigin],
) -> Option<(ProviderBinding, SelectedTargetMachineOrigin)> {
    if matches!(selected_target, Some(target) if !matches!(target, "linux_x86_64" | "linux_arm64"))
        || machine.supply_mode != psi_language_semantics::MachineSupplyMode::Boundary
        || machine.body_is_present
        || !matches!(
            machine.name.as_str(),
            "ConsoleNativeProvider::exit_process" | "ConsoleNativeProvider::write_byte"
        )
        || machine.attached_data.as_ref().map(|name| name.as_str()) != Some("ConsoleNativeProvider")
        || !machine.lifetime_parameters.is_empty()
        || !typed.machine_type_parameters(machine).is_empty()
        || conformance.name.as_str() != "Console"
        || !matches!(
            conformance.requirement.as_ref().map(|name| name.as_str()),
            Some("exit_process" | "write_byte")
        )
        || conformance.external_binding.is_some()
        || conformance.via_expression.is_valid()
        || conformance.external_binding_source_span.is_some()
    {
        return None;
    }
    let origins = target_machine_origins
        .iter()
        .filter(|origin| {
            origin.machine == machine.symbol
                && match selected_target {
                    Some(target) => origin.target == target,
                    None => matches!(origin.target.as_str(), "linux_x86_64" | "linux_arm64"),
                }
        })
        .collect::<Vec<_>>();
    let [origin] = origins.as_slice() else {
        return None;
    };
    let psi_typed_trees::machine::SatisfiedDeclaration::Trait {
        definition,
        requirement,
    } = psi_typed_trees::machine::resolve_satisfied_declaration(typed, machine, conformance)?
    else {
        return None;
    };
    if !definition.is_boundary
        || definition.symbol != conformance.symbol
        || definition.name.as_str() != "Console"
        || !definition.lifetime_parameters.is_empty()
        || !typed.trait_type_parameters(definition).is_empty()
        || requirement.symbol != conformance.requirement_symbol
        || !matches!(requirement.name.as_str(), "exit_process" | "write_byte")
        || machine
            .name
            .as_str()
            .strip_prefix("ConsoleNativeProvider::")
            != Some(requirement.name.as_str())
        || !exact_catalog_i32_to_unit_signature(
            typed,
            typed.state_signature_parameters(requirement),
            requirement.return_type,
        )
    {
        return None;
    }
    let [entry] = typed.machine_states(machine) else {
        return None;
    };
    if !exact_catalog_i32_to_unit_signature(typed, typed.state_parameters(entry), entry.return_type)
    {
        return None;
    }
    let machine = typed
        .normalized_machine_overload_identity(machine)?
        .identity();
    (!machine.is_empty()).then_some((
        ProviderBinding::CompilerIntrinsic { machine },
        (*origin).clone(),
    ))
}

fn exact_catalog_i32_to_unit_signature(
    typed: &TypedTrees,
    parameters: &[psi_typed_trees::signature::StateParameter],
    return_type: psi_typed_trees::types::TypeReferenceHandle,
) -> bool {
    let [parameter] = parameters else {
        return false;
    };
    !parameter.is_self
        && !parameter.is_const
        && !parameter.is_mutable
        && typed.primitive_type_reference(parameter.type_reference)
            == Some(psi_typed_trees::types::PrimitiveType::I32)
        && matches!(
            typed.type_reference_table.type_reference(return_type),
            psi_typed_trees::types::TypeReferenceNode::Unit
        )
}

fn external_provider_binding(
    binding: &psi_language_semantics::ExternalBindingIdentity,
    provider_type: &str,
    intrinsic_machine_identity: &str,
) -> ProviderBinding {
    use psi_language_semantics::ExternalBindingIdentity;

    match binding {
        ExternalBindingIdentity::Syscall { number } => ProviderBinding::Syscall { number: *number },
        ExternalBindingIdentity::Import { library, symbol } => {
            ProviderBinding::StringBackedImportBootstrap {
                library: library.clone(),
                symbol: symbol.clone(),
            }
        }
        ExternalBindingIdentity::CompilerIntrinsic => ProviderBinding::CompilerIntrinsic {
            machine: intrinsic_machine_identity.to_owned(),
        },
        ExternalBindingIdentity::VtableSlot { index } => {
            ProviderBinding::VtableSlot { index: *index }
        }
        ExternalBindingIdentity::VtableField { field } => ProviderBinding::VtableField {
            table: provider_type.to_owned(),
            field: field.clone(),
        },
        ExternalBindingIdentity::TableFunction { field } => ProviderBinding::TableFunction {
            table: provider_type.to_owned(),
            field: field.clone(),
        },
    }
}

fn realization_machine_identity(typed: &TypedTrees, machine_name: &str) -> String {
    typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .and_then(|machine| typed.normalized_machine_overload_identity(machine))
        .map(|identity| identity.identity())
        .unwrap_or_default()
}

fn provider_boundary_arguments(
    typed: &TypedTrees,
    boundary: &psi_typed_trees::trait_definition::TraitDefinition,
    provider_type: &str,
) -> Vec<psi_typed_trees::types::TypeReferenceHandle> {
    typed
        .conformances()
        .iter()
        .find(|conformance| {
            conformance
                .carrier_name()
                .is_some_and(|carrier| same_semantic_name(carrier.as_str(), provider_type))
                && same_semantic_name(conformance.trait_name.as_str(), boundary.name.as_str())
        })
        .map(|conformance| {
            typed
                .type_reference_table
                .type_reference_handles(conformance.arguments)
                .to_vec()
        })
        .unwrap_or_default()
}

pub(super) fn same_semantic_name(left: &str, right: &str) -> bool {
    left == right
        || (!left.contains("::") && right.rsplit("::").next().is_some_and(|leaf| leaf == left))
        || (!right.contains("::") && left.rsplit("::").next().is_some_and(|leaf| leaf == right))
}

/// The stable name shared by derivation, reports, selection, and backend row
/// extraction. External leaves may use the anonymous form; a real provider
/// type is deliberately visible in artifact identity.
pub fn satisfies_plan_name(target: &str, trait_name: &str, provider_type: &str) -> String {
    match (target.is_empty(), provider_type.is_empty()) {
        (true, true) => format!("satisfies::{trait_name}"),
        (false, true) => format!("{target}::satisfies::{trait_name}"),
        (true, false) => format!("{provider_type}::satisfies::{trait_name}"),
        (false, false) => format!("{target}::{provider_type}::satisfies::{trait_name}"),
    }
}

fn checked_adapter_has_exact_conformance(
    typed: &TypedTrees,
    adapter: &psi_typed_trees::machine::Machine,
    plan: &omega_effects::provider_plan::ProviderPlan,
    row: &omega_effects::provider_plan::ProviderPlanRow,
) -> bool {
    let top_level_requirement = typed.machines().iter().find(|requirement| {
        requirement.supply_mode == psi_language_semantics::MachineSupplyMode::TopLevelRequirement
            && ServiceSchema::from_typed_boundary_requirement(typed, requirement).as_ref()
                == Some(&plan.schema)
    });
    if let Some(requirement) = top_level_requirement {
        let identity = typed
            .normalized_machine_overload_identity(requirement)
            .map(|identity| identity.identity())
            .unwrap_or_default();
        return row.requirement_identity == identity
            && typed
                .machine_trait_conformances(adapter)
                .iter()
                .any(|conformance| {
                    conformance.external_binding.is_none()
                        && conformance.symbol == requirement.symbol
                        && conformance.requirement_symbol == requirement.symbol
                        && matches!(
                            psi_typed_trees::machine::resolve_satisfied_declaration(
                                typed,
                                adapter,
                                conformance,
                            ),
                            Some(
                                psi_typed_trees::machine::SatisfiedDeclaration::TopLevelRequirement(
                                    selected,
                                ),
                            ) if selected.symbol == requirement.symbol
                        )
                });
    }

    let operator = typed.operators().iter().find(|operator| {
        operator.is_boundary
            && psi_typed_trees::operator::boundary_operator_requirement_identity(typed, operator)
                == plan.schema.trait_name
    });
    if let Some(operator) = operator {
        let identity =
            psi_typed_trees::operator::boundary_operator_requirement_identity(typed, operator);
        let [namespace, requirement] = typed.operator_path_members(operator.name) else {
            return false;
        };
        return row.method == "realize"
            && row.requirement_identity == identity
            && typed
                .machine_trait_conformances(adapter)
                .iter()
                .any(|conformance| {
                    conformance.external_binding.is_none()
                        && conformance.name.as_str() == namespace.as_str()
                        && conformance.requirement.as_ref().map(|name| name.as_str())
                            == Some(requirement.as_str())
                        && psi_typed_trees::operator::resolve_satisfied_checked_operator(
                            typed,
                            adapter,
                            namespace.as_str(),
                            requirement.as_str(),
                        )
                        .is_some_and(|resolved| resolved.symbol == operator.symbol)
                });
    }

    typed
        .machine_trait_conformances(adapter)
        .iter()
        .filter(|conformance| conformance.external_binding.is_none())
        .filter_map(|conformance| {
            let requirement = conformance.requirement.as_ref()?;
            let definition = typed
                .traits()
                .iter()
                .find(|definition| definition.symbol == conformance.symbol)?;
            Some(satisfied_requirement_identity(
                typed,
                adapter.name.as_str(),
                definition.name.as_str(),
                requirement.as_str(),
            ))
        })
        .any(|identity| identity == row.requirement_identity)
}

fn exact_schema_method_for_row<'plan>(
    plan: &'plan ProviderPlan,
    row: &ProviderPlanRow,
) -> Result<&'plan omega_effects::provider_plan::ServiceMethod, psi_diagnostics::Diagnostic> {
    if row.requirement_identity.is_empty() {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` row `{}` has no exact synchronous-invocation overload identity",
            plan.name, row.method,
        )));
    }
    let methods = plan
        .schema
        .methods
        .iter()
        .filter(|method| plan.schema.row_binds_method(row, method))
        .collect::<Vec<_>>();
    let [method] = methods.as_slice() else {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` row `{}` / `{}` binds {} exact synchronous-invocation schema methods",
            plan.name,
            row.method,
            row.requirement_identity,
            methods.len(),
        )));
    };
    Ok(*method)
}

pub(super) fn exact_canonical_provider_schema(
    typed: &TypedTrees,
    plan: &ProviderPlan,
) -> Result<ServiceSchema, psi_diagnostics::Diagnostic> {
    if plan.schema.trait_name.is_empty() {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` has no exact canonical typed schema identity",
            plan.name,
        )));
    }

    let trait_matches = typed
        .traits()
        .iter()
        .filter(|definition| {
            definition.is_boundary && definition.name.as_str() == plan.schema.trait_name
        })
        .collect::<Vec<_>>();
    let operator_matches = typed
        .operators()
        .iter()
        .filter(|operator| {
            operator.is_boundary
                && psi_typed_trees::operator::boundary_operator_requirement_identity(
                    typed, operator,
                ) == plan.schema.trait_name
        })
        .collect::<Vec<_>>();
    let requirement_matches = typed
        .machines()
        .iter()
        .filter(|requirement| {
            requirement.supply_mode
                == psi_language_semantics::MachineSupplyMode::TopLevelRequirement
                && ServiceSchema::from_typed_boundary_requirement(typed, requirement)
                    .is_some_and(|schema| schema.trait_name == plan.schema.trait_name)
        })
        .collect::<Vec<_>>();

    match (
        trait_matches.as_slice(),
        requirement_matches.as_slice(),
        operator_matches.as_slice(),
    ) {
        ([definition], [], []) => {
            let argument_matches = typed
                .conformances()
                .iter()
                .filter(|conformance| {
                    conformance
                        .carrier_name()
                        .is_some_and(|carrier| carrier.as_str() == plan.provider_type)
                        && conformance.trait_name.as_str() == definition.name.as_str()
                })
                .collect::<Vec<_>>();
            let arguments = match argument_matches.as_slice() {
                [] => Vec::new(),
                [conformance] => typed
                    .type_reference_table
                    .type_reference_handles(conformance.arguments)
                    .to_vec(),
                _ => {
                    return Err(psi_diagnostics::Diagnostic::error(format!(
                        "ProviderPlan `{}` provider `{}` resolves to {} exact carrier argument rows for canonical typed schema `{}`",
                        plan.name,
                        plan.provider_type,
                        argument_matches.len(),
                        plan.schema.trait_name,
                    )));
                }
            };
            ServiceSchema::from_typed_instance(typed, definition, &arguments).ok_or_else(|| {
                psi_diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` exact schema `{}` did not reconstruct as a canonical typed boundary schema",
                    plan.name, plan.schema.trait_name,
                ))
            })
        }
        ([], [requirement], []) => {
            ServiceSchema::from_typed_boundary_requirement(typed, requirement).ok_or_else(|| {
                psi_diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` exact schema `{}` did not reconstruct as a canonical typed top-level boundary-requirement schema",
                    plan.name, plan.schema.trait_name,
                ))
            })
        }
        ([], [], [operator]) => ServiceSchema::from_typed_operator(typed, operator).ok_or_else(|| {
            psi_diagnostics::Diagnostic::error(format!(
                "ProviderPlan `{}` exact schema `{}` did not reconstruct as a canonical typed boundary-operator schema",
                plan.name, plan.schema.trait_name,
            ))
        }),
        _ => Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` exact schema `{}` resolves to {} canonical typed boundary traits, {} top-level boundary requirements, and {} canonical typed boundary operators",
            plan.name,
            plan.schema.trait_name,
            trait_matches.len(),
            requirement_matches.len(),
            operator_matches.len(),
        ))),
    }
}

pub(super) fn exact_row_for_schema_method<'plan>(
    plan: &'plan ProviderPlan,
    method: &omega_effects::provider_plan::ServiceMethod,
) -> Result<&'plan ProviderPlanRow, psi_diagnostics::Diagnostic> {
    if method.requirement_identity.is_empty() {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` schema method `{}` has no exact synchronous-invocation overload identity",
            plan.name, method.name,
        )));
    }
    let method_count = plan
        .schema
        .methods
        .iter()
        .filter(|candidate| candidate.requirement_identity == method.requirement_identity)
        .count();
    if method_count != 1 {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` contains {method_count} schema methods for exact synchronous-invocation overload `{}`",
            plan.name, method.requirement_identity,
        )));
    }
    let rows = plan
        .rows
        .iter()
        .filter(|row| plan.schema.row_binds_method(row, method))
        .collect::<Vec<_>>();
    let [row] = rows.as_slice() else {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` schema method `{}` / `{}` binds {} exact synchronous-invocation rows",
            plan.name,
            method.name,
            method.requirement_identity,
            rows.len(),
        )));
    };
    Ok(*row)
}

pub fn exact_checked_adapter<'typed>(
    typed: &'typed TypedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
) -> Result<&'typed psi_typed_trees::machine::Machine, psi_diagnostics::Diagnostic> {
    let ProviderBinding::CheckedAdapter {
        machine_identity,
        machine_package_identity,
    } = &row.binding
    else {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` row `{}` is not a checked-adapter binding",
            plan.name, row.requirement_identity,
        )));
    };
    if machine_identity.is_empty() {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "checked adapter for ProviderPlan `{}` row `{}` has no complete machine identity",
            plan.name, row.requirement_identity,
        )));
    }
    if *machine_package_identity != plan.origin_package_identity {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "checked adapter `{machine_identity}` for ProviderPlan `{}` does not belong to the package realizing the plan",
            plan.name,
        )));
    }
    let identity_matches = typed
        .machines()
        .iter()
        .filter(|candidate| {
            typed
                .normalized_machine_overload_identity(candidate)
                .is_some_and(|identity| identity.identity() == *machine_identity)
        })
        .collect::<Vec<_>>();
    let matches = identity_matches
        .iter()
        .copied()
        .filter(|candidate| {
            typed.symbols.symbol_package_identity(candidate.symbol) == *machine_package_identity
        })
        .collect::<Vec<_>>();
    let adapter = match matches.as_slice() {
        [adapter] => *adapter,
        [] if identity_matches.is_empty() => {
            return Err(psi_diagnostics::Diagnostic::error(format!(
                "checked adapter `{machine_identity}` for `{}::{}` is absent from typed machines",
                plan.schema.trait_name, row.method,
            )));
        }
        [] => {
            return Err(psi_diagnostics::Diagnostic::error(format!(
                "checked adapter `{machine_identity}` for ProviderPlan `{}` does not belong to its retained package identity",
                plan.name,
            )));
        }
        _ => {
            return Err(psi_diagnostics::Diagnostic::error(format!(
                "checked adapter `{machine_identity}` for ProviderPlan `{}` row `{}` resolves to {} exact typed machines",
                plan.name,
                row.requirement_identity,
                matches.len(),
            )));
        }
    };
    if !adapter.symbol.is_valid() {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "checked adapter `{machine_identity}` for ProviderPlan `{}` has no exact typed machine symbol",
            plan.name,
        )));
    }
    let actual_package_identity = typed.symbols.symbol_package_identity(adapter.symbol);
    if actual_package_identity != *machine_package_identity {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "checked adapter `{machine_identity}` for ProviderPlan `{}` does not belong to its retained package identity",
            plan.name,
        )));
    }
    validate_exact_requirement_lifetime_partition(typed, plan, row, adapter)?;
    Ok(adapter)
}

fn validate_exact_requirement_lifetime_partition(
    typed: &TypedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    realization: &psi_typed_trees::machine::Machine,
) -> Result<(), psi_diagnostics::Diagnostic> {
    let matching = typed
        .machine_trait_conformances(realization)
        .iter()
        .filter_map(|conformance| {
            let psi_typed_trees::machine::SatisfiedDeclaration::Trait {
                definition,
                requirement,
            } = psi_typed_trees::machine::resolve_satisfied_declaration(
                typed,
                realization,
                conformance,
            )?
            else {
                return None;
            };
            (typed
                .normalized_trait_requirement_overload_identity(definition, requirement)
                .identity()
                == row.requirement_identity)
                .then_some(conformance)
        })
        .collect::<Vec<_>>();
    match matching.as_slice() {
        [] if row.requirement_lifetime_partition.is_empty() => Ok(()),
        [conformance]
            if psi_typed_trees::machine::normalize_requirement_lifetime_partition(
                &conformance.trait_lifetime_arguments,
            ) == row.requirement_lifetime_partition =>
        {
            Ok(())
        }
        [] => Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` row `{}` retains a requirement lifetime partition without one exact trait realization edge",
            plan.name, row.requirement_identity,
        ))),
        [_] => Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` row `{}` requirement lifetime partition differs from its exact realization edge",
            plan.name, row.requirement_identity,
        ))),
        _ => Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` row `{}` resolves to {} exact trait realization lifetime applications",
            plan.name,
            row.requirement_identity,
            matching.len(),
        ))),
    }
}

fn exact_top_level_external_realization<'typed>(
    typed: &'typed TypedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
) -> Result<&'typed psi_typed_trees::machine::Machine, psi_diagnostics::Diagnostic> {
    let requirements = typed
        .machines()
        .iter()
        .filter(|requirement| {
            requirement.supply_mode
                == psi_language_semantics::MachineSupplyMode::TopLevelRequirement
                && ServiceSchema::from_typed_boundary_requirement(typed, requirement).as_ref()
                    == Some(&plan.schema)
                && typed
                    .normalized_machine_overload_identity(requirement)
                    .is_some_and(|identity| identity.identity() == row.requirement_identity)
        })
        .collect::<Vec<_>>();
    let [requirement] = requirements.as_slice() else {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "external ProviderPlan `{}` row `{}` resolves to {} exact top-level boundary requirements",
            plan.name,
            row.requirement_identity,
            requirements.len(),
        )));
    };
    let realizations = typed
        .machines()
        .iter()
        .filter(|machine| {
            machine
                .attached_data
                .as_ref()
                .is_some_and(|owner| owner.as_str() == plan.provider_type)
                && typed.symbols.symbol_package_identity(machine.symbol)
                    == plan.origin_package_identity
                && !machine.body_is_present
        })
        .filter(|machine| {
            let psi_language_semantics::MachineSupplyMode::ExternalRealization {
                binding: Some(supply_binding),
                mechanism: Some(supply_mechanism),
            } = machine.supply_mode
            else {
                return false;
            };
            typed
                .machine_trait_conformances(machine)
                .iter()
                .any(|conformance| {
                    if conformance.symbol != requirement.symbol
                        || conformance.requirement_symbol != requirement.symbol
                        || conformance.external_binding != Some(supply_binding)
                    {
                        return false;
                    }
                    let Some(binding) = typed.external_bindings.identity(supply_binding) else {
                        return false;
                    };
                    if binding.mechanism() != supply_mechanism {
                        return false;
                    }
                    external_provider_binding(
                        binding,
                        &plan.provider_type,
                        &realization_machine_identity(typed, machine.name.as_str()),
                    ) == row.binding
                })
        })
        .collect::<Vec<_>>();
    let [realization] = realizations.as_slice() else {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "external ProviderPlan `{}` row `{}` resolves to {} exact typed external realizations with binding `{:?}`",
            plan.name,
            row.requirement_identity,
            realizations.len(),
            row.binding,
        )));
    };
    Ok(*realization)
}

fn has_exact_top_level_ordinary_realization(
    typed: &TypedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
) -> bool {
    let requirements = typed
        .machines()
        .iter()
        .filter(|requirement| {
            requirement.supply_mode
                == psi_language_semantics::MachineSupplyMode::TopLevelRequirement
                && ServiceSchema::from_typed_boundary_requirement(typed, requirement).as_ref()
                    == Some(&plan.schema)
                && typed
                    .normalized_machine_overload_identity(requirement)
                    .is_some_and(|identity| identity.identity() == row.requirement_identity)
        })
        .collect::<Vec<_>>();
    let [requirement] = requirements.as_slice() else {
        return false;
    };
    typed
        .machines()
        .iter()
        .filter(|machine| {
            machine
                .attached_data
                .as_ref()
                .is_some_and(|owner| owner.as_str() == plan.provider_type)
                && typed.symbols.symbol_package_identity(machine.symbol)
                    == plan.origin_package_identity
                && !machine.body_is_present
                && matches!(
                    machine.supply_mode,
                    psi_language_semantics::MachineSupplyMode::ExternalRealization {
                        binding: None,
                        mechanism: None,
                    }
                )
                && typed
                    .machine_trait_conformances(machine)
                    .iter()
                    .any(|conformance| {
                        conformance.symbol == requirement.symbol
                            && conformance.requirement_symbol == requirement.symbol
                            && conformance.external_binding.is_none()
                            && conformance.via_expression.is_valid()
                    })
        })
        .count()
        == 1
}

fn exact_invocation_service_name(
    typed: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    target: psi_effects::InvocationTarget,
) -> Result<String, psi_diagnostics::Diagnostic> {
    let symbol = match target {
        psi_effects::InvocationTarget::Parameter(index) => {
            let Some(entry) = typed.machine_states(machine).first() else {
                return Err(psi_diagnostics::Diagnostic::error(format!(
                    "checked adapter `{}` has no entry state for synchronous-invocation parameter {index}",
                    machine.name,
                )));
            };
            let Ok(parameter_index) = usize::try_from(index) else {
                return Err(psi_diagnostics::Diagnostic::error(format!(
                    "checked adapter `{}` synchronous-invocation parameter index {index} is outside the target index range",
                    machine.name,
                )));
            };
            let Some(parameter) = typed
                .state_parameters(entry)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .nth(parameter_index)
            else {
                return Err(psi_diagnostics::Diagnostic::error(format!(
                    "checked adapter `{}` has no exact non-self synchronous-invocation parameter {index}",
                    machine.name,
                )));
            };
            if !parameter.type_reference.is_valid() {
                return Err(psi_diagnostics::Diagnostic::error(format!(
                    "checked adapter `{}` synchronous-invocation parameter {index} has no exact type reference",
                    machine.name,
                )));
            }
            typed
                .type_reference_table
                .type_reference(parameter.type_reference)
                .type_symbol(&typed.type_reference_table)
        }
        psi_effects::InvocationTarget::Service(symbol) => symbol,
    };
    if !symbol.is_valid() {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "checked adapter `{}` has an invalid exact synchronous-invocation service symbol",
            machine.name,
        )));
    }
    let matches = typed
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary && definition.symbol == symbol)
        .collect::<Vec<_>>();
    let [definition] = matches.as_slice() else {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "checked adapter `{}` synchronous-invocation symbol {:?} resolves to {} exact boundary traits",
            machine.name,
            symbol,
            matches.len(),
        )));
    };
    Ok(definition.name.as_str().to_owned())
}

pub(super) fn exact_checked_adapter_invocations(
    typed: &TypedTrees,
    inferred: &psi_effects::InvocationInferencePlan,
    plan: &ProviderPlan,
    method: &omega_effects::provider_plan::ServiceMethod,
    row: &ProviderPlanRow,
) -> Result<Vec<String>, psi_diagnostics::Diagnostic> {
    let ProviderBinding::CheckedAdapter {
        machine_identity, ..
    } = &row.binding
    else {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` row `{}` is not a checked-adapter binding",
            plan.name, row.requirement_identity,
        )));
    };
    let adapter = exact_checked_adapter(typed, plan, row)?;
    let summaries = inferred
        .machines
        .iter()
        .filter(|summary| summary.machine == adapter.symbol)
        .collect::<Vec<_>>();
    let [summary] = summaries.as_slice() else {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "checked adapter `{machine_identity}` resolves to {} exact synchronous-invocation inference summaries",
            summaries.len(),
        )));
    };
    let top_level_requirement = typed.machines().iter().find(|requirement| {
        requirement.supply_mode == psi_language_semantics::MachineSupplyMode::TopLevelRequirement
            && typed
                .normalized_machine_overload_identity(requirement)
                .is_some_and(|identity| identity.identity() == method.requirement_identity)
    });
    let boundaries = typed
        .traits()
        .iter()
        .filter(|definition| {
            definition.is_boundary && definition.name.as_str() == method.requirement_owner
        })
        .collect::<Vec<_>>();
    let boundary = match (top_level_requirement, boundaries.as_slice()) {
        (Some(_), []) => None,
        (None, [boundary]) => Some(*boundary),
        (None, []) => {
            let operators = typed
                .operators()
                .iter()
                .filter(|operator| {
                    operator.is_boundary
                        && psi_typed_trees::operator::boundary_operator_requirement_identity(
                            typed, operator,
                        ) == method.requirement_owner
                })
                .count();
            if operators == 1 {
                None
            } else {
                return Err(psi_diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` requirement owner `{}` resolves to neither one exact boundary trait nor one exact boundary operator for synchronous invocation",
                    plan.name, method.requirement_owner,
                )));
            }
        }
        (_, boundaries) => {
            return Err(psi_diagnostics::Diagnostic::error(format!(
                "ProviderPlan `{}` requirement owner `{}` resolves to {} exact boundary traits for self-forwarded synchronous invocation",
                plan.name,
                method.requirement_owner,
                boundaries.len(),
            )));
        }
    };

    let mut names = Vec::new();
    for target in &summary.inferred_transitive {
        let target_name = exact_invocation_service_name(typed, adapter, *target)?;
        let self_forwarded = *target == psi_effects::InvocationTarget::Parameter(0)
            && boundary.is_some_and(|boundary| {
                let parameter_count = typed
                    .machine_states(adapter)
                    .first()
                    .map(|entry| {
                        typed
                            .state_parameters(entry)
                            .iter()
                            .filter(|parameter| !parameter.is_self)
                            .count()
                    })
                    .unwrap_or_default();
                method.parameter_count.checked_add(1) == Some(parameter_count)
                    && target_name == boundary.name.as_str()
            });
        if self_forwarded {
            continue;
        }
        names.push(target_name);
    }
    names.sort_unstable();
    names.dedup();
    Ok(names)
}

pub(super) fn exact_authored_invocations(
    plan: &ProviderPlan,
    method: &omega_effects::provider_plan::ServiceMethod,
) -> Result<Vec<String>, psi_diagnostics::Diagnostic> {
    if method
        .synchronous_invocations
        .iter()
        .any(|target| target.is_empty())
    {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` exact overload `{}` has an empty synchronous-invocation identity",
            plan.name, method.requirement_identity,
        )));
    }
    if method
        .synchronous_invocations
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` exact overload `{}` synchronous-invocation identities are not strictly increasing",
            plan.name, method.requirement_identity,
        )));
    }
    Ok(method.synchronous_invocations.clone())
}

/// Validate every derived candidate before coverage and selection. A partial
/// candidate may wait for more conformances, but duplicate/stray rows and
/// malformed binding shapes are invalid in their own right. The freely
/// constructible retained schema must first equal the canonical typed schema;
/// only then may checked-adapter reach be compared with its public ceiling.
/// Independent operational refinement is validated by the machine-conformance
/// checker that produced the candidate.
pub fn validate_provider_plan_candidates(
    typed: &TypedTrees,
    plans: &[omega_effects::provider_plan::ProviderPlan],
) -> Vec<psi_diagnostics::Diagnostic> {
    let mut diagnostics = Vec::new();
    let effect_plan = psi_effects::infer_operational_may(typed);
    let service_reach_plan = psi_effects::infer_service_reaches(typed, &effect_plan);
    let invocation_plan = psi_effects::infer_synchronous_invocations(typed);
    for plan in plans {
        let structural_diagnostics = plan.validate_candidate_against_schema();
        if structural_diagnostics.is_empty() {
            let canonical_schema = match exact_canonical_provider_schema(typed, plan) {
                Ok(schema) => schema,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            if plan.schema != canonical_schema {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` retained schema `{}` does not equal its exact canonical typed schema",
                    plan.name, plan.schema.trait_name,
                )));
                continue;
            }
        }
        diagnostics.extend(
            structural_diagnostics
                .into_iter()
                .map(psi_diagnostics::Diagnostic::error),
        );
        for row in &plan.rows {
            let ProviderBinding::CheckedAdapter {
                machine_identity, ..
            } = &row.binding
            else {
                let is_top_level_requirement_plan = typed.machines().iter().any(|requirement| {
                    requirement.supply_mode
                        == psi_language_semantics::MachineSupplyMode::TopLevelRequirement
                        && ServiceSchema::from_typed_boundary_requirement(typed, requirement)
                            .as_ref()
                            == Some(&plan.schema)
                });
                let is_retained_ordinary_binding =
                    matches!(row.binding, ProviderBinding::Import { .. })
                        || matches!(row.binding, ProviderBinding::Syscall { .. })
                            && has_exact_top_level_ordinary_realization(typed, plan, row);
                if is_top_level_requirement_plan
                    && !is_retained_ordinary_binding
                    && let Err(diagnostic) = exact_top_level_external_realization(typed, plan, row)
                {
                    diagnostics.push(diagnostic);
                }
                continue;
            };
            let method = match exact_schema_method_for_row(plan, row) {
                Ok(method) => method,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            let adapter = match exact_checked_adapter(typed, plan, row) {
                Ok(adapter) => adapter,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            if adapter.attached_data.as_ref().map(|owner| owner.as_str())
                != Some(plan.provider_type.as_str())
            {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "checked adapter `{machine_identity}` for `{}::{}` belongs to provider `{}`, not selected provider `{}`",
                    plan.schema.trait_name,
                    row.method,
                    adapter
                        .attached_data
                        .as_ref()
                        .map_or("<none>", |owner| owner.as_str()),
                    plan.provider_type,
                )));
                continue;
            }
            if adapter.supply_mode != psi_language_semantics::MachineSupplyMode::CheckedBody
                || typed.machine_states(adapter).is_empty()
            {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "checked adapter `{machine_identity}` for `{}::{}` does not name a checked body with an entry state",
                    plan.schema.trait_name, row.method,
                )));
                continue;
            }
            let has_exact_conformance =
                checked_adapter_has_exact_conformance(typed, adapter, plan, row);
            if !has_exact_conformance {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "checked adapter `{machine_identity}` for `{}::{}` has no exact checked satisfies edge for requirement identity `{}`",
                    plan.schema.trait_name, row.method, row.requirement_identity,
                )));
                continue;
            }
            let service_ceiling = method.service_reach.as_slice();
            let invocation_ceiling = method.synchronous_invocations.as_slice();
            let hidden_invocations =
                match exact_checked_adapter_invocations(typed, &invocation_plan, plan, method, row)
                {
                    Ok(invocations) => invocations
                        .into_iter()
                        .filter(|target| !invocation_ceiling.contains(target))
                        .collect::<Vec<_>>(),
                    Err(diagnostic) => {
                        diagnostics.push(diagnostic);
                        Vec::new()
                    }
                };
            if !hidden_invocations.is_empty() {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "adapter `{}` does not refine `{}::{}`: its body may synchronously invoke boundary binding(s) [{}], but the requirement omits those `invokes` edges",
                    machine_identity,
                    plan.schema.trait_name,
                    row.method,
                    hidden_invocations.join(", "),
                )));
            }
            let hidden_services = service_reach_plan
                .for_machine(adapter.symbol)
                .into_iter()
                .flat_map(|summary| service_reach_plan.services(summary.effective).iter())
                .filter_map(|service| typed.service_reaches.definition(*service))
                .map(|definition| definition.name.as_str())
                .filter(|name| {
                    !service_ceiling
                        .iter()
                        .any(|allowed| allowed.as_str() == *name)
                })
                .collect::<Vec<_>>();
            if !hidden_services.is_empty() {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "adapter `{}` does not refine `{}::{}`: its body reaches boundary service(s) [{}] outside the requirement's declared service ceiling [{}] -- the satisfied requirement is the public contract; widen it or drop the service reach",
                    machine_identity,
                    plan.schema.trait_name,
                    row.method,
                    hidden_services.join(", "),
                    service_ceiling.join(", "),
                )));
            }
        }
    }
    diagnostics
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExactProviderRequirementKind {
    Trait { owner: psi_symbols::SymbolHandle },
    TopLevelRequirement,
    Operator,
}

fn exact_provider_requirement_kind(
    typed: &TypedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    requirement_symbol: psi_symbols::SymbolHandle,
) -> Result<ExactProviderRequirementKind, psi_diagnostics::Diagnostic> {
    let mut matches = Vec::new();
    for definition in typed
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
    {
        if typed
            .trait_machine_signatures(definition)
            .iter()
            .any(|requirement| {
                requirement.symbol == requirement_symbol
                    && typed
                        .normalized_trait_requirement_overload_identity(definition, requirement)
                        .identity()
                        == row.requirement_identity
            })
        {
            matches.push(ExactProviderRequirementKind::Trait {
                owner: definition.symbol,
            });
        }
    }
    if typed.machines().iter().any(|requirement| {
        requirement.symbol == requirement_symbol
            && requirement.supply_mode
                == psi_language_semantics::MachineSupplyMode::TopLevelRequirement
            && typed
                .normalized_machine_overload_identity(requirement)
                .is_some_and(|identity| identity.identity() == row.requirement_identity)
    }) {
        matches.push(ExactProviderRequirementKind::TopLevelRequirement);
    }
    if typed.operators().iter().any(|operator| {
        operator.is_boundary
            && operator.symbol == requirement_symbol
            && psi_typed_trees::operator::boundary_operator_requirement_identity(typed, operator)
                == row.requirement_identity
    }) {
        matches.push(ExactProviderRequirementKind::Operator);
    }
    let [kind] = matches.as_slice() else {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` row `{}` resolves retained requirement symbol {:?} to {} exact boundary declarations",
            plan.name,
            row.requirement_identity,
            requirement_symbol,
            matches.len(),
        )));
    };
    Ok(*kind)
}

fn exact_provider_row_conformance<'typed>(
    typed: &'typed TypedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    realization: &'typed psi_typed_trees::machine::Machine,
    requirement_symbol: psi_symbols::SymbolHandle,
    requirement_kind: ExactProviderRequirementKind,
) -> Result<&'typed psi_typed_trees::machine::TraitConformance, psi_diagnostics::Diagnostic> {
    let conformances = typed
        .machine_trait_conformances(realization)
        .iter()
        .filter(|conformance| conformance.requirement_symbol == requirement_symbol)
        .filter(|conformance| match requirement_kind {
            ExactProviderRequirementKind::Trait { owner } => matches!(
                psi_typed_trees::machine::resolve_satisfied_declaration(
                    typed,
                    realization,
                    conformance,
                ),
                Some(psi_typed_trees::machine::SatisfiedDeclaration::Trait {
                    definition,
                    requirement,
                }) if definition.symbol == owner && requirement.symbol == requirement_symbol
            ),
            ExactProviderRequirementKind::TopLevelRequirement => matches!(
                psi_typed_trees::machine::resolve_satisfied_declaration(
                    typed,
                    realization,
                    conformance,
                ),
                Some(
                    psi_typed_trees::machine::SatisfiedDeclaration::TopLevelRequirement(
                        requirement,
                    ),
                ) if requirement.symbol == requirement_symbol
            ),
            ExactProviderRequirementKind::Operator => conformance
                .requirement
                .as_ref()
                .and_then(|requirement| {
                    psi_typed_trees::operator::resolve_satisfied_boundary_operator(
                        typed,
                        realization,
                        conformance.name.as_str(),
                        requirement.as_str(),
                    )
                })
                .is_some_and(|operator| operator.symbol == requirement_symbol),
        })
        .collect::<Vec<_>>();
    let [conformance] = conformances.as_slice() else {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` row `{}` realization {:?} resolves to {} exact satisfies edges for retained requirement {:?}",
            plan.name,
            row.requirement_identity,
            realization.symbol,
            conformances.len(),
            requirement_symbol,
        )));
    };
    Ok(*conformance)
}

fn exact_provenance_schema(
    typed: &TypedTrees,
    plan: &ProviderPlan,
    provenance: &ProviderPlanProvenance,
) -> Result<ServiceSchema, psi_diagnostics::Diagnostic> {
    match provenance.schema {
        ProviderSchemaDeclaration::BoundaryTrait(symbol) => {
            let definitions = typed
                .traits()
                .iter()
                .filter(|definition| definition.symbol == symbol && definition.is_boundary)
                .collect::<Vec<_>>();
            let [definition] = definitions.as_slice() else {
                return Err(psi_diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` provenance schema symbol {:?} resolves to {} exact boundary traits",
                    plan.name,
                    symbol,
                    definitions.len(),
                )));
            };
            let arguments = provider_boundary_arguments(typed, definition, &plan.provider_type);
            ServiceSchema::from_typed_instance(typed, definition, &arguments).ok_or_else(|| {
                psi_diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` provenance did not reconstruct exact boundary trait `{}`",
                    plan.name, plan.schema.trait_name,
                ))
            })
        }
        ProviderSchemaDeclaration::BoundaryRequirement(symbol) => {
            let requirements = typed
                .machines()
                .iter()
                .filter(|requirement| {
                    requirement.symbol == symbol
                        && requirement.supply_mode
                            == psi_language_semantics::MachineSupplyMode::TopLevelRequirement
                })
                .collect::<Vec<_>>();
            let [requirement] = requirements.as_slice() else {
                return Err(psi_diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` provenance schema symbol {:?} resolves to {} exact top-level boundary requirements",
                    plan.name,
                    symbol,
                    requirements.len(),
                )));
            };
            ServiceSchema::from_typed_boundary_requirement(typed, requirement).ok_or_else(|| {
                psi_diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` provenance did not reconstruct exact top-level boundary requirement `{}`",
                    plan.name, plan.schema.trait_name,
                ))
            })
        }
        ProviderSchemaDeclaration::BoundaryOperator(symbol) => {
            let operators = typed
                .operators()
                .iter()
                .filter(|operator| operator.symbol == symbol && operator.is_boundary)
                .collect::<Vec<_>>();
            let [operator] = operators.as_slice() else {
                return Err(psi_diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` provenance schema symbol {:?} resolves to {} exact boundary operators",
                    plan.name,
                    symbol,
                    operators.len(),
                )));
            };
            ServiceSchema::from_typed_operator(typed, operator).ok_or_else(|| {
                psi_diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` provenance did not reconstruct exact boundary operator `{}`",
                    plan.name, plan.schema.trait_name,
                ))
            })
        }
    }
}

fn exact_provenance_realization<'typed>(
    typed: &'typed TypedTrees,
    plan: &ProviderPlan,
    provenance: &ProviderPlanProvenance,
    realization_symbol: psi_symbols::SymbolHandle,
) -> Result<&'typed psi_typed_trees::machine::Machine, psi_diagnostics::Diagnostic> {
    let realizations = typed
        .machines()
        .iter()
        .filter(|machine| machine.symbol == realization_symbol)
        .collect::<Vec<_>>();
    let [realization] = realizations.as_slice() else {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` retained realization symbol {:?} resolves to {} exact typed machines",
            plan.name,
            realization_symbol,
            realizations.len(),
        )));
    };
    if typed.symbols.symbol_package_identity(realization.symbol) != plan.origin_package_identity {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` retained realization {:?} does not belong to its origin package",
            plan.name, realization.symbol,
        )));
    }
    match (plan.provider_type.is_empty(), provenance.provider_type) {
        (true, None)
            if realization.attached_data.is_none()
                && !realization.attached_data_symbol.is_valid() => {}
        (false, Some(provider_symbol)) => {
            let providers = typed
                .data_definitions()
                .iter()
                .filter(|definition| {
                    definition.symbol == provider_symbol
                        && definition.name.as_str() == plan.provider_type
                        && typed.symbols.symbol_package_identity(definition.symbol)
                            == plan.provider_type_package_identity
                })
                .collect::<Vec<_>>();
            if providers.len() != 1
                || realization.attached_data_symbol != provider_symbol
                || realization
                    .attached_data
                    .as_ref()
                    .is_none_or(|name| name.as_str() != plan.provider_type)
            {
                return Err(psi_diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` retained realization {:?} does not rejoin its exact nominal provider provenance",
                    plan.name, realization.symbol,
                )));
            }
        }
        _ => {
            return Err(psi_diagnostics::Diagnostic::error(format!(
                "ProviderPlan `{}` retained realization {:?} disagrees with its nominal provider provenance",
                plan.name, realization.symbol,
            )));
        }
    }
    Ok(*realization)
}

fn replay_provider_row_binding(
    typed: &TypedTrees,
    evaluated_bindings: &crate::evaluated_via_bindings::EvaluatedViaBindingTable,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    realization: &psi_typed_trees::machine::Machine,
    conformance: &psi_typed_trees::machine::TraitConformance,
    target_machine_origin: Option<&SelectedTargetMachineOrigin>,
) -> Result<(), psi_diagnostics::Diagnostic> {
    match &row.binding {
        ProviderBinding::CheckedAdapter { .. } => {
            let adapter = exact_checked_adapter(typed, plan, row)?;
            if target_machine_origin.is_some()
                || adapter.symbol != realization.symbol
                || adapter.supply_mode != psi_language_semantics::MachineSupplyMode::CheckedBody
                || !adapter.body_is_present
                || conformance.external_binding.is_some()
                || conformance.via_expression.is_valid()
                || conformance.external_binding_source_span.is_some()
            {
                return Err(psi_diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` row `{}` does not replay its exact checked-adapter realization {:?}",
                    plan.name, row.requirement_identity, realization.symbol,
                )));
            }
        }
        _ => {
            if realization.body_is_present {
                return Err(psi_diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` row `{}` retains a body-bearing external realization",
                    plan.name, row.requirement_identity,
                )));
            }
            let replayed = match (realization.supply_mode, conformance.external_binding) {
                (psi_language_semantics::MachineSupplyMode::Boundary, None)
                    if !conformance.via_expression.is_valid()
                        && conformance.external_binding_source_span.is_none() =>
                {
                    let Some(origin) = target_machine_origin else {
                        return Err(psi_diagnostics::Diagnostic::error(format!(
                            "ProviderPlan `{}` row `{}` lost its selected target-machine origin",
                            plan.name, row.requirement_identity,
                        )));
                    };
                    let (binding, replayed_origin) = inferred_linux_console_compiler_intrinsic(
                        typed,
                        realization,
                        conformance,
                        (!plan.target.is_empty()).then_some(plan.target.as_str()),
                        std::slice::from_ref(origin),
                    )
                    .ok_or_else(|| {
                        psi_diagnostics::Diagnostic::error(format!(
                            "ProviderPlan `{}` row `{}` has no exact source-inferred compiler catalog candidate",
                            plan.name, row.requirement_identity,
                        ))
                    })?;
                    if &replayed_origin != origin {
                        return Err(psi_diagnostics::Diagnostic::error(format!(
                            "ProviderPlan `{}` row `{}` selected target-machine origin drifted",
                            plan.name, row.requirement_identity,
                        )));
                    }
                    binding
                }
                (
                    psi_language_semantics::MachineSupplyMode::ExternalRealization {
                        binding: Some(supply_binding),
                        mechanism: Some(supply_mechanism),
                    },
                    Some(conformance_binding),
                ) if supply_binding == conformance_binding
                    && !conformance.via_expression.is_valid()
                    && conformance.external_binding_source_span.is_some() =>
                {
                    if target_machine_origin.is_some() {
                        return Err(psi_diagnostics::Diagnostic::error(format!(
                            "ProviderPlan `{}` row `{}` assigns target-machine origin to legacy external supply",
                            plan.name, row.requirement_identity,
                        )));
                    }
                    let Some(binding) = typed.external_bindings.identity(supply_binding) else {
                        return Err(psi_diagnostics::Diagnostic::error(format!(
                            "ProviderPlan `{}` row `{}` has no exact legacy external binding identity",
                            plan.name, row.requirement_identity,
                        )));
                    };
                    if binding.mechanism() != supply_mechanism {
                        return Err(psi_diagnostics::Diagnostic::error(format!(
                            "ProviderPlan `{}` row `{}` legacy binding mechanism drifted",
                            plan.name, row.requirement_identity,
                        )));
                    }
                    external_provider_binding(
                        binding,
                        &plan.provider_type,
                        &typed
                            .normalized_machine_overload_identity(realization)
                            .map(|identity| identity.identity())
                            .unwrap_or_default(),
                    )
                }
                (
                    psi_language_semantics::MachineSupplyMode::ExternalRealization {
                        binding: None,
                        mechanism: None,
                    },
                    None,
                ) if conformance.via_expression.is_valid()
                    && conformance.external_binding_source_span.is_some() =>
                {
                    if target_machine_origin.is_some() {
                        return Err(psi_diagnostics::Diagnostic::error(format!(
                            "ProviderPlan `{}` row `{}` assigns target-machine origin to evaluated `via` supply",
                            plan.name, row.requirement_identity,
                        )));
                    }
                    let Some(evaluated) = evaluated_bindings.exact(
                        realization.symbol,
                        conformance.symbol,
                        conformance.requirement_symbol,
                    ) else {
                        return Err(psi_diagnostics::Diagnostic::error(format!(
                            "ProviderPlan `{}` row `{}` has no exact evaluated `via` binding row",
                            plan.name, row.requirement_identity,
                        )));
                    };
                    if evaluated.via_expression() != conformance.via_expression {
                        return Err(psi_diagnostics::Diagnostic::error(format!(
                            "ProviderPlan `{}` row `{}` evaluated `via` expression was substituted",
                            plan.name, row.requirement_identity,
                        )));
                    }
                    evaluated.evaluated().provider_binding()
                }
                _ => {
                    return Err(psi_diagnostics::Diagnostic::error(format!(
                        "ProviderPlan `{}` row `{}` has a mixed or incomplete external realization carrier",
                        plan.name, row.requirement_identity,
                    )));
                }
            };
            if replayed != row.binding {
                return Err(psi_diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` row `{}` binding does not equal its exact typed realization replay",
                    plan.name, row.requirement_identity,
                )));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_derived_provider_plan_provenance(
    typed: &TypedTrees,
    evaluated_bindings: &crate::evaluated_via_bindings::EvaluatedViaBindingTable,
    derived: &DerivedProviderPlan,
) -> Vec<psi_diagnostics::Diagnostic> {
    let plan = &derived.plan;
    let provenance = &derived.provenance;
    let mut diagnostics = Vec::new();
    match exact_provenance_schema(typed, plan, provenance) {
        Ok(schema) if schema == plan.schema => {}
        Ok(_) => diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` does not equal its exact provenance-selected typed schema",
            plan.name,
        ))),
        Err(diagnostic) => diagnostics.push(diagnostic),
    }
    if provenance.row_requirements.len() != plan.rows.len()
        || provenance.row_realizations.len() != plan.rows.len()
        || provenance.row_target_machine_origins.len() != plan.rows.len()
    {
        diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` has {} rows, {} retained requirement symbols, {} retained realization symbols, and {} target-machine origins",
            plan.name,
            plan.rows.len(),
            provenance.row_requirements.len(),
            provenance.row_realizations.len(),
            provenance.row_target_machine_origins.len(),
        )));
        return diagnostics;
    }
    for (row_index, row) in plan.rows.iter().enumerate() {
        let requirement_symbol = provenance.row_requirements[row_index];
        let realization_symbol = provenance.row_realizations[row_index];
        let kind = match exact_provider_requirement_kind(typed, plan, row, requirement_symbol) {
            Ok(kind) => kind,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };
        let category_matches_schema = matches!(
            (provenance.schema, kind),
            (
                ProviderSchemaDeclaration::BoundaryTrait(_),
                ExactProviderRequirementKind::Trait { .. },
            ) | (
                ProviderSchemaDeclaration::BoundaryRequirement(_),
                ExactProviderRequirementKind::TopLevelRequirement,
            ) | (
                ProviderSchemaDeclaration::BoundaryOperator(_),
                ExactProviderRequirementKind::Operator,
            )
        );
        if !category_matches_schema {
            diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "ProviderPlan `{}` row `{}` requirement category disagrees with its provenance schema",
                plan.name, row.requirement_identity,
            )));
            continue;
        }
        let realization =
            match exact_provenance_realization(typed, plan, provenance, realization_symbol) {
                Ok(realization) => realization,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
        let conformance = match exact_provider_row_conformance(
            typed,
            plan,
            row,
            realization,
            requirement_symbol,
            kind,
        ) {
            Ok(conformance) => conformance,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };
        if let Err(diagnostic) = replay_provider_row_binding(
            typed,
            evaluated_bindings,
            plan,
            row,
            realization,
            conformance,
            provenance.row_target_machine_origins[row_index].as_ref(),
        ) {
            diagnostics.push(diagnostic);
        }
    }
    diagnostics
}

/// Strict production validation over semantic candidates and the exact
/// derivation provenance/table custody that produced every row.
pub fn validate_derived_provider_plan_candidates(
    typed: &TypedTrees,
    evaluated_bindings: &crate::evaluated_via_bindings::EvaluatedViaBindingTable,
    plans: &[DerivedProviderPlan],
) -> Vec<psi_diagnostics::Diagnostic> {
    let mut diagnostics = evaluated_bindings
        .validate_against_typed(typed)
        .err()
        .unwrap_or_default();
    let retained_target = evaluated_bindings
        .target()
        .map(omega_target::TargetProfile::target_name)
        .unwrap_or_default();
    diagnostics.extend(validate_provider_plan_candidates(
        typed,
        &plans
            .iter()
            .map(|derived| derived.plan.clone())
            .collect::<Vec<_>>(),
    ));
    for derived in plans {
        if derived.plan.target != retained_target {
            diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "ProviderPlan `{}` target `{}` disagrees with evaluated-binding target `{retained_target}`",
                derived.plan.name, derived.plan.target,
            )));
        }
        diagnostics.extend(validate_derived_provider_plan_provenance(
            typed,
            evaluated_bindings,
            derived,
        ));
    }
    for evaluated in evaluated_bindings.rows() {
        let retained = plans.iter().any(|derived| {
            derived
                .plan
                .rows
                .iter()
                .zip(&derived.provenance.row_requirements)
                .zip(&derived.provenance.row_realizations)
                .any(|((row, requirement), realization)| {
                    *realization == evaluated.realization_machine()
                        && *requirement == evaluated.requirement()
                        && row.binding == evaluated.evaluated().provider_binding()
                })
        });
        if !retained {
            diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "evaluated `via` row for realization {:?} and requirement {:?} was not retained by any exact derived ProviderPlan provenance row",
                evaluated.realization_machine(),
                evaluated.requirement(),
            )));
        }
    }
    diagnostics
}
