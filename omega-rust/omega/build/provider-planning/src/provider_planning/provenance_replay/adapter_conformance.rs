//! Exact conformance of checked adapters to provider schemas, with the
//! invocations and realizations that prove it.

use crate::provider_planning::provenance_replay::requirement_identities::{
    exact_satisfied_requirement_identity, external_provider_binding, realization_machine_identity,
};
use crate::provider_planning::{
    ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema, TypedTrees,
};

pub(crate) fn checked_adapter_has_exact_conformance(
    typed: &TypedTrees,
    adapter: &typed_trees::machine::Machine,
    plan: &effects::provider_plan::ProviderPlan,
    row: &effects::provider_plan::ProviderPlanRow,
) -> bool {
    let top_level_requirement = typed.machines().iter().find(|requirement| {
        requirement.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
            && crate::service_schema::from_typed_boundary_requirement(typed, requirement).as_ref()
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
                            typed_trees::machine::resolve_satisfied_declaration(
                                typed,
                                adapter,
                                conformance,
                            ),
                            Some(
                                typed_trees::machine::SatisfiedDeclaration::TopLevelRequirement(
                                    selected,
                                ),
                            ) if selected.symbol == requirement.symbol
                        )
                });
    }

    let operator = typed.operators().iter().find(|operator| {
        crate::service_schema::schema_binds_exact_boundary_operator(typed, &plan.schema, operator)
    });
    if let Some(operator) = operator {
        let identity =
            typed_trees::operator::boundary_operator_requirement_identity(typed, operator);
        return row.method == "realize"
            && row.requirement_identity == identity
            && typed
                .machine_trait_conformances(adapter)
                .iter()
                .any(|conformance| {
                    conformance.external_binding.is_none()
                        && typed_trees::operator::resolve_satisfied_checked_operator_for_conformance(
                            typed,
                            adapter,
                            conformance,
                        )
                        .is_some_and(|resolved| resolved.symbol == operator.symbol)
                });
    }

    typed
        .machine_trait_conformances(adapter)
        .iter()
        .filter(|conformance| conformance.external_binding.is_none())
        .filter_map(|conformance| {
            conformance.requirement.as_ref()?;
            Some(exact_satisfied_requirement_identity(
                typed,
                conformance.symbol,
                conformance.requirement_symbol,
            ))
        })
        .any(|identity| identity == row.requirement_identity)
}

pub(crate) fn exact_schema_method_for_row<'plan>(
    plan: &'plan ProviderPlan,
    row: &ProviderPlanRow,
) -> Result<&'plan effects::provider_plan::ServiceMethod, diagnostics::Diagnostic> {
    if row.requirement_identity.is_empty() {
        return Err(diagnostics::Diagnostic::error(format!(
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
        return Err(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` row `{}` / `{}` binds {} exact synchronous-invocation schema methods",
            plan.name,
            row.method,
            row.requirement_identity,
            methods.len(),
        )));
    };
    Ok(*method)
}

pub(crate) fn exact_canonical_provider_schema(
    typed: &TypedTrees,
    plan: &ProviderPlan,
) -> Result<ServiceSchema, diagnostics::Diagnostic> {
    if plan.schema.trait_name.is_empty() {
        return Err(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` has no exact canonical typed schema identity",
            plan.name,
        )));
    }

    // Every schema kind rejoins on the exact retained package identity: the
    // readable schema name is deliberately package-blind, so same-spelled
    // declarations in another package must not widen or satisfy the match.
    let trait_matches = typed
        .traits()
        .iter()
        .filter(|definition| {
            crate::service_schema::schema_binds_exact_boundary_trait(
                typed,
                &plan.schema,
                definition,
            )
        })
        .collect::<Vec<_>>();
    let operator_matches = typed
        .operators()
        .iter()
        .filter(|operator| {
            crate::service_schema::schema_binds_exact_boundary_operator(
                typed,
                &plan.schema,
                operator,
            )
        })
        .collect::<Vec<_>>();
    let requirement_matches = typed
        .machines()
        .iter()
        .filter(|requirement| {
            crate::service_schema::schema_binds_exact_boundary_requirement(
                typed,
                &plan.schema,
                requirement,
            )
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
                        && conformance.trait_symbol == definition.symbol
                        && crate::service_schema::is_product_declaration(typed, conformance.carrier_symbol)
                        && typed.symbols.symbol_package_identity(conformance.carrier_symbol)
                            == plan.provider_type_package_identity
                })
                .collect::<Vec<_>>();
            let arguments = match argument_matches.as_slice() {
                [] => Vec::new(),
                [conformance] => typed
                    .type_reference_table
                    .type_reference_handles(conformance.arguments)
                    .to_vec(),
                _ => {
                    return Err(diagnostics::Diagnostic::error(format!(
                        "ProviderPlan `{}` provider `{}` resolves to {} exact carrier argument rows for canonical typed schema `{}`",
                        plan.name,
                        plan.provider_type,
                        argument_matches.len(),
                        plan.schema.trait_name,
                    )));
                }
            };
            crate::service_schema::from_typed_instance(typed, definition, &arguments).ok_or_else(|| {
                diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` exact schema `{}` did not reconstruct as a canonical typed boundary schema",
                    plan.name, plan.schema.trait_name,
                ))
            })
        }
        ([], [requirement], []) => {
            crate::service_schema::from_typed_boundary_requirement(typed, requirement).ok_or_else(|| {
                diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` exact schema `{}` did not reconstruct as a canonical typed top-level boundary-requirement schema",
                    plan.name, plan.schema.trait_name,
                ))
            })
        }
        ([], [], [operator]) => crate::service_schema::from_typed_operator(typed, operator).ok_or_else(|| {
            diagnostics::Diagnostic::error(format!(
                "ProviderPlan `{}` exact schema `{}` did not reconstruct as a canonical typed boundary-operator schema",
                plan.name, plan.schema.trait_name,
            ))
        }),
        _ => Err(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` exact schema `{}` resolves to {} canonical typed boundary traits, {} top-level boundary requirements, and {} canonical typed boundary operators",
            plan.name,
            plan.schema.trait_name,
            trait_matches.len(),
            requirement_matches.len(),
            operator_matches.len(),
        ))),
    }
}

pub(crate) fn exact_row_for_schema_method<'plan>(
    plan: &'plan ProviderPlan,
    method: &effects::provider_plan::ServiceMethod,
) -> Result<&'plan ProviderPlanRow, diagnostics::Diagnostic> {
    if method.requirement_identity.is_empty() {
        return Err(diagnostics::Diagnostic::error(format!(
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
        return Err(diagnostics::Diagnostic::error(format!(
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
        return Err(diagnostics::Diagnostic::error(format!(
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
) -> Result<&'typed typed_trees::machine::Machine, diagnostics::Diagnostic> {
    let ProviderBinding::CheckedAdapter {
        machine_identity,
        machine_package_identity,
    } = &row.binding
    else {
        return Err(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` row `{}` is not a checked-adapter binding",
            plan.name, row.requirement_identity,
        )));
    };
    if machine_identity.is_empty() {
        return Err(diagnostics::Diagnostic::error(format!(
            "checked adapter for ProviderPlan `{}` row `{}` has no complete machine identity",
            plan.name, row.requirement_identity,
        )));
    }
    if *machine_package_identity != plan.origin_package_identity {
        return Err(diagnostics::Diagnostic::error(format!(
            "checked adapter `{machine_identity}` for ProviderPlan `{}` does not belong to the package realizing the plan",
            plan.name,
        )));
    }
    let identity_matches = typed
        .machines()
        .iter()
        .filter(|candidate| {
            crate::service_schema::is_product_declaration(typed, candidate.symbol)
                && typed
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
            return Err(diagnostics::Diagnostic::error(format!(
                "checked adapter `{machine_identity}` for `{}::{}` is absent from typed machines",
                plan.schema.trait_name, row.method,
            )));
        }
        [] => {
            return Err(diagnostics::Diagnostic::error(format!(
                "checked adapter `{machine_identity}` for ProviderPlan `{}` does not belong to its retained package identity",
                plan.name,
            )));
        }
        _ => {
            return Err(diagnostics::Diagnostic::error(format!(
                "checked adapter `{machine_identity}` for ProviderPlan `{}` row `{}` resolves to {} exact typed machines",
                plan.name,
                row.requirement_identity,
                matches.len(),
            )));
        }
    };
    if !adapter.symbol.is_valid() {
        return Err(diagnostics::Diagnostic::error(format!(
            "checked adapter `{machine_identity}` for ProviderPlan `{}` has no exact typed machine symbol",
            plan.name,
        )));
    }
    let actual_package_identity = typed.symbols.symbol_package_identity(adapter.symbol);
    if actual_package_identity != *machine_package_identity {
        return Err(diagnostics::Diagnostic::error(format!(
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
    realization: &typed_trees::machine::Machine,
) -> Result<(), diagnostics::Diagnostic> {
    let matching = typed
        .machine_trait_conformances(realization)
        .iter()
        .filter_map(|conformance| {
            let typed_trees::machine::SatisfiedDeclaration::Trait {
                definition,
                requirement,
            } = typed_trees::machine::resolve_satisfied_declaration(
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
            if typed_trees::machine::normalize_requirement_lifetime_partition(
                &conformance.trait_lifetime_arguments,
            ) == row.requirement_lifetime_partition =>
        {
            Ok(())
        }
        [] => Err(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` row `{}` retains a requirement lifetime partition without one exact trait realization edge",
            plan.name, row.requirement_identity,
        ))),
        [_] => Err(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` row `{}` requirement lifetime partition differs from its exact realization edge",
            plan.name, row.requirement_identity,
        ))),
        _ => Err(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` row `{}` resolves to {} exact trait realization lifetime applications",
            plan.name,
            row.requirement_identity,
            matching.len(),
        ))),
    }
}

pub(crate) fn exact_top_level_external_realization<'typed>(
    typed: &'typed TypedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
) -> Result<&'typed typed_trees::machine::Machine, diagnostics::Diagnostic> {
    let requirements = typed
        .machines()
        .iter()
        .filter(|requirement| {
            requirement.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
                && crate::service_schema::from_typed_boundary_requirement(typed, requirement)
                    .as_ref()
                    == Some(&plan.schema)
                && typed
                    .normalized_machine_overload_identity(requirement)
                    .is_some_and(|identity| identity.identity() == row.requirement_identity)
        })
        .collect::<Vec<_>>();
    let [requirement] = requirements.as_slice() else {
        return Err(diagnostics::Diagnostic::error(format!(
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
            let language_semantics::MachineSupplyMode::ExternalRealization {
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
                        &realization_machine_identity(typed, machine),
                    ) == row.binding
                })
        })
        .collect::<Vec<_>>();
    let [realization] = realizations.as_slice() else {
        return Err(diagnostics::Diagnostic::error(format!(
            "external ProviderPlan `{}` row `{}` resolves to {} exact typed external realizations with binding `{:?}`",
            plan.name,
            row.requirement_identity,
            realizations.len(),
            row.binding,
        )));
    };
    Ok(*realization)
}

pub(crate) fn has_exact_top_level_ordinary_realization(
    typed: &TypedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
) -> bool {
    let requirements = typed
        .machines()
        .iter()
        .filter(|requirement| {
            requirement.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
                && crate::service_schema::from_typed_boundary_requirement(typed, requirement)
                    .as_ref()
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
                    language_semantics::MachineSupplyMode::ExternalRealization {
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
    machine: &typed_trees::machine::Machine,
    target: flow_effects::InvocationTarget,
) -> Result<String, diagnostics::Diagnostic> {
    let symbol = match target {
        flow_effects::InvocationTarget::Parameter(index) => {
            let Some(entry) = typed.machine_states(machine).first() else {
                return Err(diagnostics::Diagnostic::error(format!(
                    "checked adapter `{}` has no entry state for synchronous-invocation parameter {index}",
                    machine.name,
                )));
            };
            let Ok(parameter_index) = usize::try_from(index) else {
                return Err(diagnostics::Diagnostic::error(format!(
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
                return Err(diagnostics::Diagnostic::error(format!(
                    "checked adapter `{}` has no exact non-self synchronous-invocation parameter {index}",
                    machine.name,
                )));
            };
            if !parameter.type_reference.is_valid() {
                return Err(diagnostics::Diagnostic::error(format!(
                    "checked adapter `{}` synchronous-invocation parameter {index} has no exact type reference",
                    machine.name,
                )));
            }
            typed
                .type_reference_table
                .type_reference(parameter.type_reference)
                .type_symbol(&typed.type_reference_table)
        }
        flow_effects::InvocationTarget::Service(symbol) => symbol,
    };
    if !symbol.is_valid() {
        return Err(diagnostics::Diagnostic::error(format!(
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
        return Err(diagnostics::Diagnostic::error(format!(
            "checked adapter `{}` synchronous-invocation symbol {:?} resolves to {} exact boundary traits",
            machine.name,
            symbol,
            matches.len(),
        )));
    };
    Ok(typed.trait_declaration_path(definition))
}

pub(crate) fn exact_checked_adapter_invocations(
    typed: &TypedTrees,
    inferred: &flow_effects::InvocationInferencePlan,
    plan: &ProviderPlan,
    method: &effects::provider_plan::ServiceMethod,
    row: &ProviderPlanRow,
) -> Result<Vec<String>, diagnostics::Diagnostic> {
    let ProviderBinding::CheckedAdapter {
        machine_identity, ..
    } = &row.binding
    else {
        return Err(diagnostics::Diagnostic::error(format!(
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
        return Err(diagnostics::Diagnostic::error(format!(
            "checked adapter `{machine_identity}` resolves to {} exact synchronous-invocation inference summaries",
            summaries.len(),
        )));
    };
    let top_level_requirement = typed.machines().iter().find(|requirement| {
        requirement.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
            && typed
                .normalized_machine_overload_identity(requirement)
                .is_some_and(|identity| identity.identity() == method.requirement_identity)
            && crate::service_schema::is_product_declaration(typed, requirement.symbol)
            && typed.symbols.symbol_package_identity(requirement.symbol)
                == method.requirement_owner_package_identity
    });
    let boundaries = typed
        .traits()
        .iter()
        .filter(|definition| {
            definition.is_boundary
                && crate::service_schema::is_product_declaration(typed, definition.symbol)
                && typed.trait_declaration_path(definition) == method.requirement_owner
                && typed.symbols.symbol_package_identity(definition.symbol)
                    == method.requirement_owner_package_identity
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
                        && crate::service_schema::is_product_declaration(typed, operator.symbol)
                        && typed_trees::operator::boundary_operator_requirement_identity(
                            typed, operator,
                        ) == method.requirement_owner
                        && typed.symbols.symbol_package_identity(operator.symbol)
                            == method.requirement_owner_package_identity
                })
                .count();
            if operators == 1 {
                None
            } else {
                return Err(diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` requirement owner `{}` resolves to neither one exact boundary trait nor one exact boundary operator for synchronous invocation",
                    plan.name, method.requirement_owner,
                )));
            }
        }
        (_, boundaries) => {
            return Err(diagnostics::Diagnostic::error(format!(
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
        let self_forwarded = *target == flow_effects::InvocationTarget::Parameter(0)
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
                    && target_name == typed.trait_declaration_path(boundary)
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

pub(crate) fn exact_authored_invocations(
    plan: &ProviderPlan,
    method: &effects::provider_plan::ServiceMethod,
) -> Result<Vec<String>, diagnostics::Diagnostic> {
    if method
        .synchronous_invocations
        .iter()
        .any(|target| target.is_empty())
    {
        return Err(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` exact overload `{}` has an empty synchronous-invocation identity",
            plan.name, method.requirement_identity,
        )));
    }
    if method
        .synchronous_invocations
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` exact overload `{}` synchronous-invocation identities are not strictly increasing",
            plan.name, method.requirement_identity,
        )));
    }
    Ok(method.synchronous_invocations.clone())
}
