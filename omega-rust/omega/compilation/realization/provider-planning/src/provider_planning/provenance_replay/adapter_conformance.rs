//! Exact checked adapters of selected provider rows, the schema rows they
//! realize and the boundary bindings they synchronously invoke. Every
//! declaration join uses the caller's explicit planning context; portable
//! identity cannot select a scope.

use crate::ProviderPlanningContext;
use crate::provider_planning::{ProviderBinding, ProviderPlan, ProviderPlanRow, trait_name_text};
use checked_trees::typed_trees::service_declarations::{ProviderSchemaDeclaration, ServiceCatalog};

pub(crate) use backend_plan::synchronous_invocations::exact_row_for_schema_method;

/// Join one selected requirement to its exact
/// checked adapter. Absence means the declaration has no boundary schema or
/// the selected realization is not a checked body; malformed authority rejects.
/// A retained requirement outside the supplied context rejects rather than
/// appearing to have no schema.
/// Execution admissibility and source operands remain the interpreter's work.
pub fn selected_requirement_checked_adapter<'typed>(
    services: ServiceCatalog<'typed>,
    plan: &ProviderPlan,
    requirement: symbols::SymbolHandle,
) -> Result<
    Option<(
        &'typed checked_trees::typed_trees::machine::Machine,
        symbols::SymbolHandle,
    )>,
    diagnostics::Diagnostic,
> {
    let context = services.context();
    let typed = context.typed;
    if !context.contains(requirement) {
        return Err(diagnostics::Diagnostic::error(format!(
            "selected requirement {:?} for ProviderPlan `{}` is outside the {:?} planning context",
            requirement, plan.name, context.scope,
        )));
    }
    // Actual machines own their schemas, including token-spelled requirements.
    // A signature view must not supply a fallback schema for an invalid machine.
    let trait_method = typed.trait_machine_signature(requirement);
    let schema = if let Some(machine) = typed.machine(requirement) {
        services.declaration(ProviderSchemaDeclaration::BoundaryRequirement(
            machine.symbol,
        ))
    } else if let Some((owner, _)) = trait_method {
        services.declaration(ProviderSchemaDeclaration::BoundaryTrait(owner.symbol))
    } else {
        typed
            .operator_definition_by_symbol(requirement)
            .and_then(|operator| {
                services.declaration(ProviderSchemaDeclaration::BoundaryOperator(operator.symbol))
            })
    };
    let Some(prepared) = schema else {
        return Ok(None);
    };
    let schema = crate::service_schema::attach_calling(&prepared, &[]);
    let trait_identity = trait_method
        .map(|(owner, method)| typed.trait_requirement_callable_identity_text(owner, method));
    let methods = schema
        .declaration
        .methods
        .iter()
        .filter(|method| {
            trait_identity.as_ref().is_none_or(|identity| {
                schema.identity_text(method.requirement_identity) == identity.as_str()
            })
        })
        .collect::<Vec<_>>();
    let [required_method] = methods.as_slice() else {
        return Err(diagnostics::Diagnostic::error(
            "selected build-time requirement must identify exactly one schema method",
        ));
    };
    let overload_identity = &required_method.requirement_identity;
    if schema.identity_text(*overload_identity).is_empty() {
        return Err(diagnostics::Diagnostic::error(format!(
            "selected build-time requirement for ProviderPlan `{}` has an empty canonical overload identity",
            plan.name,
        )));
    }
    if plan.schema != schema || plan.rows.len() != schema.declaration.methods.len() {
        return Err(diagnostics::Diagnostic::error(format!(
            "selected build-time operator ProviderPlan `{}` does not bind exact overload `{}`",
            plan.name,
            schema.identity_text(*overload_identity),
        )));
    }
    // A trait owns one plan for all its methods. Select the demanded exact
    // coordinate without requiring the whole trait to be a singleton slot.
    for method in &schema.declaration.methods {
        exact_row_for_schema_method(plan, method)?;
    }
    let row = exact_row_for_schema_method(plan, required_method)?;
    let ProviderBinding::CheckedAdapter { .. } = &row.binding else {
        return Ok(None);
    };
    if plan.provider_type.is_empty() {
        return Err(diagnostics::Diagnostic::error(format!(
            "selected build-time operator ProviderPlan `{}` has no nominal provider type",
            plan.name,
        )));
    }
    let adapter = exact_checked_adapter(context, plan, row)?;
    let declaration = match crate::provider_identity::named_provider(context, plan) {
        Some(owner) => owner.symbol,
        None => adapter.attached_data_symbol,
    };
    if !declaration.is_valid() || !context.contains(declaration) {
        return Err(diagnostics::Diagnostic::error(
            "selected build-time provider lost its exact selecting declaration",
        ));
    }
    Ok(Some((adapter, declaration)))
}

/// Rejoin one active adapter within the caller's declaration context, retaining
/// exact callable, package, provider and satisfies-edge custody.
pub fn exact_checked_adapter<'typed>(
    context: ProviderPlanningContext<'typed>,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
) -> Result<&'typed checked_trees::typed_trees::machine::Machine, diagnostics::Diagnostic> {
    let typed = context.typed;
    let ProviderBinding::CheckedAdapter {
        machine_identity,
        machine_package_identity,
    } = &row.binding
    else {
        return Err(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` row `{}` is not a checked-adapter binding",
            plan.name,
            plan.identities.text(row.requirement_identity),
        )));
    };
    if plan.identity_text(*machine_identity).is_empty() {
        return Err(diagnostics::Diagnostic::error(format!(
            "checked adapter for ProviderPlan `{}` row `{}` has no complete machine identity",
            plan.name,
            plan.identities.text(row.requirement_identity),
        )));
    }
    if *machine_package_identity != plan.origin_package_identity {
        return Err(diagnostics::Diagnostic::error(format!(
            "checked adapter `{}` for ProviderPlan `{}` does not belong to the package realizing the plan",
            plan.identity_text(*machine_identity),
            plan.name,
        )));
    }
    let target_identity_text = plan.identity_text(*machine_identity);
    let target_callable_identity = typed.callable_identity_by_text(target_identity_text);
    let identity_matches = if target_callable_identity.is_valid() {
        let mut matches = typed
            .machines_by_callable_identity(target_callable_identity)
            .into_iter()
            .filter(|candidate| {
                crate::target_machines::realizes_machine(context, candidate, Some(&plan.target))
            })
            .collect::<Vec<_>>();
        for candidate in typed.machines() {
            if candidate.target.is_some()
                && crate::target_machines::realizes_machine(context, candidate, Some(&plan.target))
                && typed.machine_callable_identity(candidate) == target_callable_identity
            {
                matches.push(candidate);
            }
        }
        matches
    } else {
        Vec::new()
    };
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
                "checked adapter `{}` for `{}::{}` is absent from typed machines",
                plan.identity_text(*machine_identity),
                trait_name_text(&plan.schema),
                row.method,
            )));
        }
        [] => {
            return Err(diagnostics::Diagnostic::error(format!(
                "checked adapter `{}` for ProviderPlan `{}` does not belong to its retained package identity",
                plan.identity_text(*machine_identity),
                plan.name,
            )));
        }
        _ => {
            return Err(diagnostics::Diagnostic::error(format!(
                "checked adapter `{}` for ProviderPlan `{}` row `{}` resolves to {} exact typed machines",
                plan.identity_text(*machine_identity),
                plan.name,
                plan.identities.text(row.requirement_identity),
                matches.len(),
            )));
        }
    };
    if !adapter.symbol.is_valid() {
        return Err(diagnostics::Diagnostic::error(format!(
            "checked adapter `{}` for ProviderPlan `{}` has no exact typed machine symbol",
            plan.identity_text(*machine_identity),
            plan.name,
        )));
    }
    if adapter.attached_data_symbol.is_valid() && !context.contains(adapter.attached_data_symbol) {
        return Err(diagnostics::Diagnostic::error(format!(
            "checked adapter `{}` for ProviderPlan `{}` retains a provider outside the {:?} planning context",
            plan.identity_text(*machine_identity),
            plan.name,
            context.scope,
        )));
    }
    if typed
        .machine_trait_conformances(adapter)
        .iter()
        .any(|conformance| {
            !context.contains(conformance.symbol)
                || !context.contains(conformance.requirement_symbol)
        })
    {
        return Err(diagnostics::Diagnostic::error(format!(
            "checked adapter `{}` for ProviderPlan `{}` retains a satisfies edge outside the {:?} planning context",
            plan.identity_text(*machine_identity),
            plan.name,
            context.scope,
        )));
    }
    let actual_package_identity = typed.symbols.symbol_package_identity(adapter.symbol);
    if actual_package_identity != *machine_package_identity {
        return Err(diagnostics::Diagnostic::error(format!(
            "checked adapter `{}` for ProviderPlan `{}` does not belong to its retained package identity",
            plan.identity_text(*machine_identity),
            plan.name,
        )));
    }
    if !plan.provider_type.is_empty()
        && !crate::provider_identity::provider_row_matches(context, plan, row, adapter)
    {
        return Err(diagnostics::Diagnostic::error(format!(
            "checked adapter `{}` does not rejoin provider `{}` row `{}`",
            plan.identity_text(*machine_identity),
            plan.provider_type,
            plan.identities.text(row.requirement_identity),
        )));
    }
    validate_exact_requirement_lifetime_partition(context, plan, row, adapter)?;
    Ok(adapter)
}

fn validate_exact_requirement_lifetime_partition(
    context: ProviderPlanningContext<'_>,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    realization: &checked_trees::typed_trees::machine::Machine,
) -> Result<(), diagnostics::Diagnostic> {
    let typed = context.typed;
    // Named maps own their trait application independently of the selected
    // machine. Requiring an attached satisfies edge loses the map's regions.
    if crate::provider_identity::named_provider(context, plan).is_some() {
        let application = crate::provider_identity::named_provider_row(context, plan, row)
            .and_then(|entry| {
                crate::provider_identity::provider_trait_application(
                    context,
                    plan,
                    row,
                    realization,
                    entry.declaring_trait,
                    entry.requirement,
                )
            })
            .ok_or_else(|| {
                diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` row `{}` lost its exact named trait application",
                    plan.name,
                    plan.identities.text(row.requirement_identity),
                ))
            })?;
        return if language_semantics::type_identity::normalize_requirement_lifetime_partition(
            application.lifetime_arguments(),
        ) == row.requirement_lifetime_partition
        {
            Ok(())
        } else {
            Err(diagnostics::Diagnostic::error(format!(
                "ProviderPlan `{}` row `{}` requirement lifetime partition differs from its exact named application",
                plan.name,
                plan.identities.text(row.requirement_identity),
            )))
        };
    }
    let mut matching = Vec::new();
    for conformance in typed.machine_trait_conformances(realization) {
        let Some(checked_trees::typed_trees::machine::SatisfiedDeclaration::Trait {
            definition,
            requirement,
        }) = checked_trees::typed_trees::machine::resolve_satisfied_declaration(
            typed,
            realization,
            conformance,
        )
        else {
            continue;
        };
        if typed.trait_requirement_callable_identity_text(definition, requirement)
            != plan.identities.text(row.requirement_identity)
        {
            continue;
        }
        if !context.contains(definition.symbol) || !context.contains(requirement.symbol) {
            return Err(diagnostics::Diagnostic::error(format!(
                "ProviderPlan `{}` row `{}` requirement lifetime edge is outside the {:?} planning context",
                plan.name,
                plan.identities.text(row.requirement_identity),
                context.scope,
            )));
        }
        matching.push(conformance);
    }
    match matching.as_slice() {
        [] if row.requirement_lifetime_partition.is_empty() => Ok(()),
        [conformance]
            if language_semantics::type_identity::normalize_requirement_lifetime_partition(
                &conformance.trait_lifetime_arguments,
            ) == row.requirement_lifetime_partition =>
        {
            Ok(())
        }
        [] => Err(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` row `{}` retains a requirement lifetime partition without one exact trait realization edge",
            plan.name,
            plan.identities.text(row.requirement_identity),
        ))),
        [_] => Err(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` row `{}` requirement lifetime partition differs from its exact realization edge",
            plan.name,
            plan.identities.text(row.requirement_identity),
        ))),
        _ => Err(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` row `{}` resolves to {} exact trait realization lifetime applications",
            plan.name,
            plan.identities.text(row.requirement_identity),
            matching.len(),
        ))),
    }
}

fn exact_invocation_target(
    context: ProviderPlanningContext<'_>,
    machine: &checked_trees::typed_trees::machine::Machine,
    target: flow_effects::RealizedInvocationTarget,
) -> Result<super::super::synchronous_cycles::SynchronousBoundaryTarget, diagnostics::Diagnostic> {
    let typed = context.typed;
    let (symbol, application) = match target {
        flow_effects::RealizedInvocationTarget::Requirement(requirement) => {
            let scope = if let Some(application) = typed.closed_requirement_application(requirement) {
                application.trait_scope.scope(typed)
            } else {
                typed.trait_machine_signature(requirement).and_then(|(declaration, _)| {
                    (typed.trait_type_parameters(declaration).is_empty()
                        && declaration.lifetime_parameters.is_empty())
                        .then(|| checked_trees::typed_trees::trait_definition::AppliedTraitScope::abstract_scope(declaration))
                })
            }.ok_or_else(|| diagnostics::Diagnostic::error(format!(
                "checked adapter `{}` synchronous invocation lost its exact requirement application",
                machine.name,
            )))?;
            let application = scope.portable_application_identity(typed)
                .and_then(|identity| typed.type_identity_text(identity))
                .ok_or_else(|| diagnostics::Diagnostic::error(format!(
                    "checked adapter `{}` synchronous invocation has no portable application identity",
                    machine.name,
                )))?;
            (scope.declaration.symbol, Some(application))
        }
        flow_effects::RealizedInvocationTarget::Abstract(
            flow_effects::InvocationTarget::Service(symbol),
        ) => (symbol, None),
        flow_effects::RealizedInvocationTarget::Abstract(
            flow_effects::InvocationTarget::Parameter(index),
        ) => {
            return Err(diagnostics::Diagnostic::error(format!(
                "checked adapter `{}` retains retired runtime boundary-binding parameter {index} in its synchronous invocation graph",
                machine.name,
            )));
        }
    };
    if !symbol.is_valid() {
        return Err(diagnostics::Diagnostic::error(format!(
            "checked adapter `{}` has an invalid exact synchronous-invocation service symbol",
            machine.name,
        )));
    }
    if !context.contains(symbol) {
        return Err(diagnostics::Diagnostic::error(format!(
            "checked adapter `{}` synchronous-invocation service {:?} is outside the {:?} planning context",
            machine.name, symbol, context.scope,
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
    Ok(
        super::super::synchronous_cycles::SynchronousBoundaryTarget::Application {
            declaration: typed.trait_declaration_path(definition),
            package: typed.symbols.symbol_package_identity(symbol),
            application,
        },
    )
}

pub(crate) fn exact_checked_adapter_invocations(
    context: ProviderPlanningContext<'_>,
    invocations: &checked_trees::SynchronousInvocationFacts,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
) -> Result<Vec<super::super::synchronous_cycles::SynchronousBoundaryTarget>, diagnostics::Diagnostic>
{
    let ProviderBinding::CheckedAdapter {
        machine_identity, ..
    } = &row.binding
    else {
        return Err(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` row `{}` is not a checked-adapter binding",
            plan.name,
            plan.identities.text(row.requirement_identity),
        )));
    };
    let adapter = exact_checked_adapter(context, plan, row)?;
    let summaries = invocations
        .machines
        .iter()
        .filter(|summary| summary.machine == adapter.symbol)
        .collect::<Vec<_>>();
    let [summary] = summaries.as_slice() else {
        return Err(diagnostics::Diagnostic::error(format!(
            "checked adapter `{}` resolves to {} exact checked synchronous-invocation facts",
            plan.identity_text(*machine_identity),
            summaries.len(),
        )));
    };
    summary
        .realized_targets
        .iter()
        .map(|target| exact_invocation_target(context, adapter, *target))
        .collect()
}
