//! Provenance replay of selected provider plans against the typed program,
//! for package review. Review compiles a candidate package and then replays
//! the retained provenance of every authored selected plan before it projects
//! inert policy; the ordinary compile does not run this replay on the plans it
//! just derived (pipeline.md, "Verification placement"). Retained symbols
//! establish scope before any portable schema or binding comparison.

use crate::ProviderPlanningContext;
use crate::provider_planning::provenance_replay::requirement_identities::{
    external_provider_binding, inferred_compiler_leaf_binding, provider_boundary_arguments,
};
use crate::provider_planning::provenance_replay::{
    DerivedProviderPlan, ProviderPlanProvenance, ProviderSchemaDeclaration,
    SelectedTargetMachineOrigin, exact_checked_adapter,
};
use crate::provider_planning::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema};

/// Replay each selected plan's retained provenance: its schema, requirement
/// kinds, realizations, satisfies edges and row bindings must rejoin the
/// typed program, and its target must equal the evaluated-binding target.
pub fn replay_selected_provider_provenance(
    context: ProviderPlanningContext<'_>,
    evaluated_bindings: &crate::evaluated_via_bindings::EvaluatedViaBindingTable,
    selected: &[crate::SelectedProviderPlanWithProvenance],
) -> Vec<diagnostics::Diagnostic> {
    let retained_target = evaluated_bindings
        .target()
        .map(target::TargetProfile::target_name)
        .unwrap_or_default();
    let mut diagnostics = Vec::new();
    for selected_plan in selected {
        let plan = &selected_plan.derived.plan;
        if plan.target != retained_target {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "selected ProviderPlan `{}` target `{}` disagrees with evaluated-binding target `{retained_target}`",
                plan.name, plan.target,
            )));
        }
        diagnostics.extend(validate_derived_provider_plan_provenance(
            context,
            evaluated_bindings,
            &selected_plan.derived,
        ));
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
    context: ProviderPlanningContext<'_>,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    requirement_symbol: symbols::SymbolHandle,
) -> Result<ExactProviderRequirementKind, diagnostics::Diagnostic> {
    let typed = context.typed;
    if !context.contains(requirement_symbol) {
        return Err(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` row `{}` retained requirement {:?} is outside the {:?} planning context",
            plan.name, row.requirement_identity, requirement_symbol, context.scope,
        )));
    }
    let mut matches = Vec::new();
    for definition in typed
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary && context.contains(definition.symbol))
    {
        if typed
            .trait_machine_signatures(definition)
            .iter()
            .any(|requirement| {
                requirement.symbol == requirement_symbol
                    && typed.trait_requirement_callable_identity_text(definition, requirement)
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
                .machine_callable_identity_text(requirement)
                .is_some_and(|identity| identity == row.requirement_identity)
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
    context: ProviderPlanningContext<'typed>,
    top_level_symbols: &mut Option<validation::TopLevelSymbols<'typed>>,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    realization: &'typed typed_trees::machine::Machine,
    requirement_symbol: symbols::SymbolHandle,
    requirement_kind: ExactProviderRequirementKind,
) -> Result<&'typed typed_trees::machine::TraitConformance, diagnostics::Diagnostic> {
    let typed = context.typed;
    let conformances = typed
        .machine_trait_conformances(realization)
        .iter()
        .filter(|conformance| {
            context.contains(conformance.symbol) && context.contains(conformance.requirement_symbol)
        })
        .filter(|conformance| match requirement_kind {
            ExactProviderRequirementKind::Trait { owner } => {
                conformance.requirement_symbol == requirement_symbol
                    && matches!(
                        typed_trees::machine::resolve_satisfied_declaration(
                            typed,
                            realization,
                            conformance,
                        ),
                        Some(typed_trees::machine::SatisfiedDeclaration::Trait {
                            definition,
                            requirement,
                        }) if definition.symbol == owner && requirement.symbol == requirement_symbol
                    )
            }
            ExactProviderRequirementKind::TopLevelRequirement => {
                if matches!(
                    typed_trees::machine::resolve_satisfied_declaration(
                        typed,
                        realization,
                        conformance,
                    ),
                    Some(
                        typed_trees::machine::SatisfiedDeclaration::TopLevelRequirement(
                            requirement,
                        ),
                    ) if conformance.requirement_symbol == requirement.symbol
                        && requirement.symbol == requirement_symbol
                ) {
                    return true;
                }
                // A specialized provider instance's satisfies edge names the
                // generic requirement template; the row's requirement symbol
                // is its exact closed application. The clause's settled
                // symbols still select the template, and the specialization
                // receipt replays the join rather than aliasing the closed
                // requirement.
                let Some(template) = typed.machines().iter().find(|candidate| {
                    candidate.symbol == conformance.symbol
                        && context.contains(candidate.symbol)
                        && conformance.requirement_symbol == candidate.symbol
                        && candidate.supply_mode
                            == language_semantics::MachineSupplyMode::TopLevelRequirement
                        && !typed.machine_type_parameters(candidate).is_empty()
                }) else {
                    return false;
                };
                crate::provider_planning::provenance_replay::satisfied_row_requirement(
                    typed,
                    top_level_symbols,
                    realization,
                    template,
                )
                .is_some_and(|closed| {
                    closed.symbol == requirement_symbol && context.contains(closed.symbol)
                })
            }
            ExactProviderRequirementKind::Operator => {
                conformance.requirement_symbol == requirement_symbol
                    && typed_trees::operator::resolve_satisfied_boundary_operator_for_conformance(
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
    context: ProviderPlanningContext<'_>,
    plan: &ProviderPlan,
    provenance: &ProviderPlanProvenance,
) -> Result<ServiceSchema, diagnostics::Diagnostic> {
    let typed = context.typed;
    // Portable schemas may be identical across separately checked contexts.
    // Validate the retained symbols before reconstructing that projection,
    // including provider custody for candidates that do not yet have rows.
    if !context.contains(provenance.schema.symbol()) {
        return Err(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` retained schema {:?} is outside the {:?} planning context",
            plan.name,
            provenance.schema.symbol(),
            context.scope,
        )));
    }
    if let Some(provider_symbol) = provenance.provider_type
        && !context.contains(provider_symbol)
    {
        return Err(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` retained provider {:?} is outside the {:?} planning context",
            plan.name, provider_symbol, context.scope,
        )));
    }
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
            let arguments =
                provider_boundary_arguments(typed, definition, provenance.provider_type);
            crate::service_schema::from_typed_instance(context, definition, &arguments).ok_or_else(|| {
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
            crate::service_schema::from_typed_boundary_requirement(context, requirement).ok_or_else(|| {
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
            crate::service_schema::from_typed_operator(context, operator).ok_or_else(|| {
                diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` provenance did not reconstruct exact boundary operator `{}`",
                    plan.name, plan.schema.trait_name,
                ))
            })
        }
    }
}

fn exact_provenance_realization<'typed>(
    context: ProviderPlanningContext<'typed>,
    plan: &ProviderPlan,
    provenance: &ProviderPlanProvenance,
    row: &ProviderPlanRow,
    realization_symbol: symbols::SymbolHandle,
) -> Result<&'typed typed_trees::machine::Machine, diagnostics::Diagnostic> {
    let typed = context.typed;
    if !context.contains(realization_symbol) {
        return Err(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` retained realization {:?} is outside the {:?} planning context",
            plan.name, realization_symbol, context.scope,
        )));
    }
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
    if let Some(provider) = crate::provider_identity::named_provider(context, plan) {
        if provenance.provider_type != Some(provider.symbol)
            || typed.symbols.symbol_package_identity(provider.symbol)
                != plan.origin_package_identity
            || !crate::provider_identity::provider_row_matches(context, plan, row, realization)
        {
            return Err(diagnostics::Diagnostic::error(format!(
                "ProviderPlan `{}` realization does not rejoin its named implementation row",
                plan.name,
            )));
        }
        return Ok(*realization);
    }
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
                        && context.contains(definition.symbol)
                        && typed.data_declaration_path(definition) == plan.provider_type
                        && typed.symbols.symbol_package_identity(definition.symbol)
                            == plan.provider_type_package_identity
                })
                .collect::<Vec<_>>();
            if providers.len() != 1
                || realization.attached_data_symbol != provider_symbol
                || providers.first().is_none_or(|provider| {
                    realization.attached_data.as_ref() != Some(&provider.name)
                })
                || typed.attached_data_path(realization).as_deref()
                    != Some(plan.provider_type.as_str())
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
    context: ProviderPlanningContext<'_>,
    evaluated_bindings: &crate::evaluated_via_bindings::EvaluatedViaBindingTable,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    realization: &typed_trees::machine::Machine,
    conformance: Option<&typed_trees::machine::TraitConformance>,
    target_machine_origin: Option<&SelectedTargetMachineOrigin>,
) -> Result<(), diagnostics::Diagnostic> {
    let typed = context.typed;
    if let Some(origin) = target_machine_origin
        && !context.contains(origin.machine)
    {
        return Err(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` row `{}` retained target-machine origin {:?} is outside the {:?} planning context",
            plan.name, row.requirement_identity, origin.machine, context.scope,
        )));
    }
    match &row.binding {
        ProviderBinding::CheckedAdapter { .. } => {
            let adapter = exact_checked_adapter(context, plan, row)?;
            if target_machine_origin.is_some()
                || adapter.symbol != realization.symbol
                || adapter.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                || !adapter.body_is_present
                || conformance.is_some_and(|conformance| {
                    conformance.external_binding.is_some()
                        || conformance.via_expression.is_valid()
                        || conformance.external_binding_source_span.is_some()
                })
            {
                return Err(diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` row `{}` does not replay its exact checked-adapter realization {:?}",
                    plan.name, row.requirement_identity, realization.symbol,
                )));
            }
        }
        _ => {
            let conformance = conformance.ok_or_else(|| {
                diagnostics::Diagnostic::error(
                    "external provider binding is missing its exact realization edge",
                )
            })?;
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
                    let (binding, replayed_origin) = inferred_compiler_leaf_binding(
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
                            .machine_callable_identity_text(realization)
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
                    if evaluated.evaluated().target().target_name() != plan.target
                        || evaluated_bindings.target() != Some(evaluated.evaluated().target())
                    {
                        return Err(diagnostics::Diagnostic::error(format!(
                            "ProviderPlan `{}` row `{}` evaluated `via` target does not equal its selected target",
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

fn validate_derived_provider_plan_provenance(
    context: ProviderPlanningContext<'_>,
    evaluated_bindings: &crate::evaluated_via_bindings::EvaluatedViaBindingTable,
    derived: &DerivedProviderPlan,
) -> Vec<diagnostics::Diagnostic> {
    let plan = &derived.plan;
    let provenance = &derived.provenance;
    let mut diagnostics = Vec::new();
    match exact_provenance_schema(context, plan, provenance) {
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
    let mut top_level_symbols = None;
    for (row_index, row) in plan.rows.iter().enumerate() {
        let requirement_symbol = provenance.row_requirements[row_index];
        let realization_symbol = provenance.row_realizations[row_index];
        let kind = match exact_provider_requirement_kind(context, plan, row, requirement_symbol) {
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
        let realization = match exact_provenance_realization(
            context,
            plan,
            provenance,
            row,
            realization_symbol,
        ) {
            Ok(realization) => realization,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };
        let conformance = if crate::provider_identity::named_provider(context, plan).is_some()
            && matches!(row.binding, ProviderBinding::CheckedAdapter { .. })
        {
            None
        } else {
            Some(
                match exact_provider_row_conformance(
                    context,
                    &mut top_level_symbols,
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
                },
            )
        };
        if let Err(diagnostic) = replay_provider_row_binding(
            context,
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
