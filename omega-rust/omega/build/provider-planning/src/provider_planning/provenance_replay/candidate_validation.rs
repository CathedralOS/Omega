//! Validation of provider-plan candidates and derived plans against
//! replayed provenance.

use crate::provider_planning::provenance_replay::adapter_conformance::{
    checked_adapter_has_exact_conformance, exact_canonical_provider_schema,
    exact_checked_adapter_invocations, exact_schema_method_for_row,
    exact_top_level_external_realization, has_exact_top_level_ordinary_realization,
};
use crate::provider_planning::provenance_replay::requirement_identities::{
    external_provider_binding, inferred_hosted_console_compiler_intrinsic,
    provider_boundary_arguments,
};
use crate::provider_planning::provenance_replay::{
    DerivedProviderPlan, ProviderPlanProvenance, ProviderSchemaDeclaration,
    SelectedTargetMachineOrigin, exact_checked_adapter,
};
use crate::provider_planning::{
    ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema, TypedTrees,
};

/// Validate every derived candidate before coverage and selection. A partial
/// candidate may wait for more conformances, but duplicate/stray rows and
/// malformed binding shapes are invalid in their own right. The freely
/// constructible retained schema must first equal the canonical typed schema;
/// only then may checked-adapter reach be compared with its public ceiling.
/// Independent operational refinement is validated by the machine-conformance
/// checker that produced the candidate.
pub fn validate_provider_plan_candidates(
    typed: &TypedTrees,
    plans: &[effects::provider_plan::ProviderPlan],
) -> Vec<diagnostics::Diagnostic> {
    let mut diagnostics = Vec::new();
    let effect_plan = validation::infer_operational_may(typed);
    let service_reach_plan = validation::infer_service_reaches(typed, &effect_plan);
    let invocation_plan = validation::infer_synchronous_invocations(typed);
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
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` retained schema `{}` does not equal its exact canonical typed schema",
                    plan.name, plan.schema.trait_name,
                )));
                continue;
            }
        }
        diagnostics.extend(
            structural_diagnostics
                .into_iter()
                .map(diagnostics::Diagnostic::error),
        );
        for row in &plan.rows {
            let ProviderBinding::CheckedAdapter {
                machine_identity, ..
            } = &row.binding
            else {
                let is_top_level_requirement_plan = typed.machines().iter().any(|requirement| {
                    requirement.supply_mode
                        == language_semantics::MachineSupplyMode::TopLevelRequirement
                        && crate::service_schema::from_typed_boundary_requirement(
                            typed,
                            requirement,
                        )
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
                diagnostics.push(diagnostics::Diagnostic::error(format!(
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
            if adapter.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                || typed.machine_states(adapter).is_empty()
            {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "checked adapter `{machine_identity}` for `{}::{}` does not name a checked body with an entry state",
                    plan.schema.trait_name, row.method,
                )));
                continue;
            }
            let has_exact_conformance =
                checked_adapter_has_exact_conformance(typed, adapter, plan, row);
            if !has_exact_conformance {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
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
                diagnostics.push(diagnostics::Diagnostic::error(format!(
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
                diagnostics.push(diagnostics::Diagnostic::error(format!(
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
    Trait { owner: symbols::SymbolHandle },
    TopLevelRequirement,
    Operator,
}

fn exact_provider_requirement_kind(
    typed: &TypedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    requirement_symbol: symbols::SymbolHandle,
) -> Result<ExactProviderRequirementKind, diagnostics::Diagnostic> {
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
            && requirement.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
            && typed
                .normalized_machine_overload_identity(requirement)
                .is_some_and(|identity| identity.identity() == row.requirement_identity)
    }) {
        matches.push(ExactProviderRequirementKind::TopLevelRequirement);
    }
    if typed.operators().iter().any(|operator| {
        operator.is_boundary
            && operator.symbol == requirement_symbol
            && typed_trees::operator::boundary_operator_requirement_identity(typed, operator)
                == row.requirement_identity
    }) {
        matches.push(ExactProviderRequirementKind::Operator);
    }
    let [kind] = matches.as_slice() else {
        return Err(diagnostics::Diagnostic::error(format!(
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
    realization: &'typed typed_trees::machine::Machine,
    requirement_symbol: symbols::SymbolHandle,
    requirement_kind: ExactProviderRequirementKind,
) -> Result<&'typed typed_trees::machine::TraitConformance, diagnostics::Diagnostic> {
    let conformances = typed
        .machine_trait_conformances(realization)
        .iter()
        .filter(|conformance| conformance.requirement_symbol == requirement_symbol)
        .filter(|conformance| match requirement_kind {
            ExactProviderRequirementKind::Trait { owner } => matches!(
                typed_trees::machine::resolve_satisfied_declaration(
                    typed,
                    realization,
                    conformance,
                ),
                Some(typed_trees::machine::SatisfiedDeclaration::Trait {
                    definition,
                    requirement,
                }) if definition.symbol == owner && requirement.symbol == requirement_symbol
            ),
            ExactProviderRequirementKind::TopLevelRequirement => matches!(
                typed_trees::machine::resolve_satisfied_declaration(
                    typed,
                    realization,
                    conformance,
                ),
                Some(
                    typed_trees::machine::SatisfiedDeclaration::TopLevelRequirement(
                        requirement,
                    ),
                ) if requirement.symbol == requirement_symbol
            ),
            ExactProviderRequirementKind::Operator => {
                typed_trees::operator::resolve_satisfied_boundary_operator_for_conformance(
                    typed,
                    realization,
                    conformance,
                )
                .is_some_and(|operator| operator.symbol == requirement_symbol)
            }
        })
        .collect::<Vec<_>>();
    let [conformance] = conformances.as_slice() else {
        return Err(diagnostics::Diagnostic::error(format!(
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
) -> Result<ServiceSchema, diagnostics::Diagnostic> {
    match provenance.schema {
        ProviderSchemaDeclaration::BoundaryTrait(symbol) => {
            let definitions = typed
                .traits()
                .iter()
                .filter(|definition| definition.symbol == symbol && definition.is_boundary)
                .collect::<Vec<_>>();
            let [definition] = definitions.as_slice() else {
                return Err(diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` provenance schema symbol {:?} resolves to {} exact boundary traits",
                    plan.name,
                    symbol,
                    definitions.len(),
                )));
            };
            let arguments = provider_boundary_arguments(typed, definition, &plan.provider_type);
            crate::service_schema::from_typed_instance(typed, definition, &arguments).ok_or_else(|| {
                diagnostics::Diagnostic::error(format!(
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
                            == language_semantics::MachineSupplyMode::TopLevelRequirement
                })
                .collect::<Vec<_>>();
            let [requirement] = requirements.as_slice() else {
                return Err(diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` provenance schema symbol {:?} resolves to {} exact top-level boundary requirements",
                    plan.name,
                    symbol,
                    requirements.len(),
                )));
            };
            crate::service_schema::from_typed_boundary_requirement(typed, requirement).ok_or_else(|| {
                diagnostics::Diagnostic::error(format!(
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
                return Err(diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` provenance schema symbol {:?} resolves to {} exact boundary operators",
                    plan.name,
                    symbol,
                    operators.len(),
                )));
            };
            crate::service_schema::from_typed_operator(typed, operator).ok_or_else(|| {
                diagnostics::Diagnostic::error(format!(
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
    realization_symbol: symbols::SymbolHandle,
) -> Result<&'typed typed_trees::machine::Machine, diagnostics::Diagnostic> {
    let realizations = typed
        .machines()
        .iter()
        .filter(|machine| machine.symbol == realization_symbol)
        .collect::<Vec<_>>();
    let [realization] = realizations.as_slice() else {
        return Err(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` retained realization symbol {:?} resolves to {} exact typed machines",
            plan.name,
            realization_symbol,
            realizations.len(),
        )));
    };
    if typed.symbols.symbol_package_identity(realization.symbol) != plan.origin_package_identity {
        return Err(diagnostics::Diagnostic::error(format!(
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
                return Err(diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` retained realization {:?} does not rejoin its exact nominal provider provenance",
                    plan.name, realization.symbol,
                )));
            }
        }
        _ => {
            return Err(diagnostics::Diagnostic::error(format!(
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
    realization: &typed_trees::machine::Machine,
    conformance: &typed_trees::machine::TraitConformance,
    target_machine_origin: Option<&SelectedTargetMachineOrigin>,
) -> Result<(), diagnostics::Diagnostic> {
    match &row.binding {
        ProviderBinding::CheckedAdapter { .. } => {
            let adapter = exact_checked_adapter(typed, plan, row)?;
            if target_machine_origin.is_some()
                || adapter.symbol != realization.symbol
                || adapter.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                || !adapter.body_is_present
                || conformance.external_binding.is_some()
                || conformance.via_expression.is_valid()
                || conformance.external_binding_source_span.is_some()
            {
                return Err(diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` row `{}` does not replay its exact checked-adapter realization {:?}",
                    plan.name, row.requirement_identity, realization.symbol,
                )));
            }
        }
        _ => {
            if realization.body_is_present {
                return Err(diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` row `{}` retains a body-bearing external realization",
                    plan.name, row.requirement_identity,
                )));
            }
            let replayed = match (realization.supply_mode, conformance.external_binding) {
                (language_semantics::MachineSupplyMode::Boundary, None)
                    if !conformance.via_expression.is_valid()
                        && conformance.external_binding_source_span.is_none() =>
                {
                    let Some(origin) = target_machine_origin else {
                        return Err(diagnostics::Diagnostic::error(format!(
                            "ProviderPlan `{}` row `{}` lost its selected target-machine origin",
                            plan.name, row.requirement_identity,
                        )));
                    };
                    let (binding, replayed_origin) = inferred_hosted_console_compiler_intrinsic(
                        typed,
                        realization,
                        conformance,
                        (!plan.target.is_empty()).then_some(plan.target.as_str()),
                        std::slice::from_ref(origin),
                    )
                    .ok_or_else(|| {
                        diagnostics::Diagnostic::error(format!(
                            "ProviderPlan `{}` row `{}` has no exact source-inferred compiler catalog candidate",
                            plan.name, row.requirement_identity,
                        ))
                    })?;
                    if &replayed_origin != origin {
                        return Err(diagnostics::Diagnostic::error(format!(
                            "ProviderPlan `{}` row `{}` selected target-machine origin drifted",
                            plan.name, row.requirement_identity,
                        )));
                    }
                    binding
                }
                (
                    language_semantics::MachineSupplyMode::ExternalRealization {
                        binding: Some(supply_binding),
                        mechanism: Some(supply_mechanism),
                    },
                    Some(conformance_binding),
                ) if supply_binding == conformance_binding
                    && !conformance.via_expression.is_valid()
                    && conformance.external_binding_source_span.is_some() =>
                {
                    if target_machine_origin.is_some() {
                        return Err(diagnostics::Diagnostic::error(format!(
                            "ProviderPlan `{}` row `{}` assigns target-machine origin to legacy external supply",
                            plan.name, row.requirement_identity,
                        )));
                    }
                    let Some(binding) = typed.external_bindings.identity(supply_binding) else {
                        return Err(diagnostics::Diagnostic::error(format!(
                            "ProviderPlan `{}` row `{}` has no exact legacy external binding identity",
                            plan.name, row.requirement_identity,
                        )));
                    };
                    if binding.mechanism() != supply_mechanism {
                        return Err(diagnostics::Diagnostic::error(format!(
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
                    language_semantics::MachineSupplyMode::ExternalRealization {
                        binding: None,
                        mechanism: None,
                    },
                    None,
                ) if conformance.via_expression.is_valid()
                    && conformance.external_binding_source_span.is_some() =>
                {
                    if target_machine_origin.is_some() {
                        return Err(diagnostics::Diagnostic::error(format!(
                            "ProviderPlan `{}` row `{}` assigns target-machine origin to evaluated `via` supply",
                            plan.name, row.requirement_identity,
                        )));
                    }
                    let Some(evaluated) = evaluated_bindings.exact(
                        realization.symbol,
                        conformance.symbol,
                        conformance.requirement_symbol,
                    ) else {
                        return Err(diagnostics::Diagnostic::error(format!(
                            "ProviderPlan `{}` row `{}` has no exact evaluated `via` binding row",
                            plan.name, row.requirement_identity,
                        )));
                    };
                    if evaluated.via_expression() != conformance.via_expression {
                        return Err(diagnostics::Diagnostic::error(format!(
                            "ProviderPlan `{}` row `{}` evaluated `via` expression was substituted",
                            plan.name, row.requirement_identity,
                        )));
                    }
                    evaluated.evaluated().provider_binding()
                }
                _ => {
                    return Err(diagnostics::Diagnostic::error(format!(
                        "ProviderPlan `{}` row `{}` has a mixed or incomplete external realization carrier",
                        plan.name, row.requirement_identity,
                    )));
                }
            };
            if replayed != row.binding {
                return Err(diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` row `{}` binding does not equal its exact typed realization replay",
                    plan.name, row.requirement_identity,
                )));
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_derived_provider_plan_provenance(
    typed: &TypedTrees,
    evaluated_bindings: &crate::evaluated_via_bindings::EvaluatedViaBindingTable,
    derived: &DerivedProviderPlan,
) -> Vec<diagnostics::Diagnostic> {
    let plan = &derived.plan;
    let provenance = &derived.provenance;
    let mut diagnostics = Vec::new();
    match exact_provenance_schema(typed, plan, provenance) {
        Ok(schema) if schema == plan.schema => {}
        Ok(_) => diagnostics.push(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` does not equal its exact provenance-selected typed schema",
            plan.name,
        ))),
        Err(diagnostic) => diagnostics.push(diagnostic),
    }
    if provenance.row_requirements.len() != plan.rows.len()
        || provenance.row_realizations.len() != plan.rows.len()
        || provenance.row_target_machine_origins.len() != plan.rows.len()
    {
        diagnostics.push(diagnostics::Diagnostic::error(format!(
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
            diagnostics.push(diagnostics::Diagnostic::error(format!(
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
) -> Vec<diagnostics::Diagnostic> {
    let mut diagnostics = evaluated_bindings
        .validate_against_typed(typed)
        .err()
        .unwrap_or_default();
    let retained_target = evaluated_bindings
        .target()
        .map(target::TargetProfile::target_name)
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
            diagnostics.push(diagnostics::Diagnostic::error(format!(
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
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "evaluated `via` row for realization {:?} and requirement {:?} was not retained by any exact derived ProviderPlan provenance row",
                evaluated.realization_machine(),
                evaluated.requirement(),
            )));
        }
    }
    diagnostics
}
