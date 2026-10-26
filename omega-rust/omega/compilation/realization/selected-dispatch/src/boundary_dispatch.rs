//! Resolve checked boundary calls to exact selected adapters without rewriting
//! their source meaning. Exact and receiver-forwarding adapters share the same
//! association; execution consumes it, while Terminal retains the requirement.
//!
//! This file settles and plans selected boundary adapter dispatch.
//! `adapter_rows.rs` carries adapter rows and their resolution,
//! `signature_families.rs` probes finite signature families and
//! `boundary_fields.rs` names boundary fields, traits, receivers and
//! adapter calls. Language fixtures exercise dispatch through both engines.

mod adapter_rows;
pub(crate) mod boundary_fields;
mod signature_families;

use crate::boundary_dispatch::adapter_rows::{
    AdapterRow, GenericBoundaryRequirement, ResolvedAdapterRow, resolve_selected_adapter_row,
};
use crate::boundary_dispatch::boundary_fields::{
    BoundaryField, named_type_symbol, resolve_adapter_call,
};
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;

/// Service erasure joins the selected plan, not its checked-adapter subset.
/// Intrinsic and foreign rows deliberately produce no adapter dispatch; an
/// unrelated checked provider must not make their service carriers invalid.
/// Exact schema ownership still matters: another requirement's valid digest
/// cannot authorize this receiver. Fused selection provenance and carrier
/// custody remain independently checked by `service_custody` before native use.
fn selected_service_plan_for_application<'plans>(
    typed: &CheckedTrees,
    calling_plans: &[provider_planning::calling_policy_plans::BoundaryCallingPlanRealization],
    selected_plans: &'plans effects::SelectedProviderPlanFacts,
    application: &typed_trees::trait_definition::AppliedTraitScope<'_>,
    arguments: &[typed_trees::types::TypeReferenceHandle],
) -> Option<&'plans effects::provider_plan::ProviderPlan> {
    let schema = provider_planning::service_schema::from_typed_application(
        provider_planning::ProviderPlanningContext::new(typed, source::DependencyScope::Product),
        calling_plans,
        application.clone(),
        arguments,
    )?;
    let mut matching = selected_plans.plans().iter().filter(|plan| {
        provider_planning::provider_receiver_application(typed, plan, application, &schema)
            .is_some()
    });
    let plan = matching.next()?;
    matching.next().is_none().then_some(plan)
}

/// Produce selected execution associations without changing source-derived facts.
pub(crate) fn settle_selected_boundary_adapter_dispatch(
    checked: &CheckedTrees,
    calling_plans: &[provider_planning::calling_policy_plans::BoundaryCallingPlanRealization],
    selected_plans: &effects::SelectedProviderPlanFacts,
) -> Result<Vec<checked_trees::CheckedBoundaryAdapterDispatch>, Vec<Diagnostic>> {
    // Review retains checked Build bodies, but this operation realizes only
    // Product execution. Build's admitted provider context is independent:
    // requiring its calls to join Product plans would replay the wrong purpose.
    // Select all Product roots, not a guessed reachable subset or callee names.
    let context =
        provider_planning::ProviderPlanningContext::new(checked, source::DependencyScope::Product);
    let product_roots = typed_trees::references::DeclarationSelection::from_symbols(
        typed_trees::references::collect_program_declarations(checked)
            .into_iter()
            .filter(|symbol| context.contains(*symbol)),
    );
    // Keep canonical backing storage. Only Product declarations own dispatch
    // work; excluded Build declarations remain available for exact lookup but
    // cannot supply a Product row. Source-free generated roots are included by
    // the context above, just as in the former declaration projection.
    let typed = checked;
    let mut adapters = Vec::new();
    let mut generic_requirements: Vec<GenericBoundaryRequirement> = Vec::new();
    let mut diagnostics = Vec::new();
    for plan in selected_plans.plans() {
        for row in &plan.rows {
            match resolve_selected_adapter_row(
                typed,
                &product_roots,
                plan,
                row,
                selected_plans.specialized_requirement_rows(),
                selected_plans.identities(),
            ) {
                Ok(rows) => {
                    for resolved in rows {
                        match resolved {
                            ResolvedAdapterRow::Adapter(adapter) => {
                                if let Some(existing) =
                                    adapters.iter().find(|existing: &&AdapterRow| {
                                        existing.receiver_trait == adapter.receiver_trait
                                            && existing.requirement_symbol
                                                == adapter.requirement_symbol
                                            && existing.family_tuple == adapter.family_tuple
                                    })
                                {
                                    diagnostics.push(Diagnostic::error(format!(
                                        "selected boundary requirement `{}` tuple `({})` has two checked adapters (`{}` and `{}`)",
                                        adapter.requirement_identity,
                                        adapter.family_tuple_display.join(", "),
                                        existing.adapter_target,
                                        adapter.adapter_target,
                                    )));
                                } else {
                                    adapters.push(adapter);
                                }
                            }
                            ResolvedAdapterRow::GenericRequirement(requirement) => {
                                if !generic_requirements.iter().any(|existing| {
                                    existing.receiver_trait == requirement.receiver_trait
                                        && existing.requirement_symbol
                                            == requirement.requirement_symbol
                                        && existing.provider_plan_digest
                                            == requirement.provider_plan_digest
                                }) {
                                    generic_requirements.push(requirement);
                                }
                            }
                        }
                    }
                }
                Err(diagnostic) => diagnostics.push(diagnostic),
            }
        }
    }
    if adapters.is_empty() && generic_requirements.is_empty() {
        return diagnostics.is_empty().then(Vec::new).ok_or(diagnostics);
    }

    let product_references =
        typed_trees::references::collect_selected_program_references(typed, &product_roots);

    // Exact typed field symbol -> exact boundary-trait symbol. Field spellings
    // may repeat across data owners and never participate in dispatch.
    let mut boundary_fields = Vec::new();
    for data in typed
        .data_definitions()
        .iter()
        .filter(|data| product_roots.contains(data.symbol))
    {
        for member in typed.data_members(data) {
            let Some(field) = member.as_field() else {
                continue;
            };
            let (symbol, application, provider_plan_digest) = if let Ok(Some(carrier)) =
                typed_trees::service::classify_exact_bound_service_carrier(
                    typed,
                    typed_trees::service::ServiceCarrierOccurrence {
                        type_reference: field.type_reference,
                        lexical_owner: data.symbol,
                        enclosing_data_applications: &[],
                    },
                ) {
                let Some(application) = carrier.application(typed, data.symbol, &[]) else {
                    continue;
                };
                let Some(plan) = selected_service_plan_for_application(
                    typed,
                    calling_plans,
                    selected_plans,
                    &application,
                    carrier.arguments(typed),
                ) else {
                    // Unselected requirements supply no executable adapter.
                    // Root establishment owns missing-provider diagnostics.
                    continue;
                };
                (
                    carrier.requirement,
                    Some(application.environment()),
                    Some(*plan.identity_digest().as_bytes()),
                )
            } else {
                let Some(symbol) = typed
                    .type_reference_table
                    .type_reference(field.type_reference)
                    .as_named_symbol()
                else {
                    continue;
                };
                (symbol, None, None)
            };
            let receiver = BoundaryField {
                symbol: field.symbol,
                trait_symbol: symbol,
                application,
                provider_plan_digest,
            };
            if !adapters
                .iter()
                .any(|adapter| receiver.supplies_adapter(typed, adapter))
                && !generic_requirements.iter().any(|selected| {
                    receiver.supplies_requirement(
                        typed,
                        selected.provider_plan_digest,
                        selected.receiver_trait,
                        selected.requirement_symbol,
                    )
                })
            {
                continue;
            }
            if !field.symbol.is_valid() {
                diagnostics.push(Diagnostic::error(format!(
                    "boundary field `{}::{}` has no exact typed symbol for adapter dispatch",
                    data.name, field.name,
                )));
                continue;
            }
            if let Some(existing) = boundary_fields
                .iter()
                .find(|existing: &&BoundaryField| existing.symbol == field.symbol)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "boundary field symbol {:?} maps to both trait symbols {:?} and {:?}",
                    field.symbol, existing.trait_symbol, symbol,
                )));
                continue;
            }
            boundary_fields.push(receiver);
        }
    }

    // Retained nominal qualifiers join direct requirements without a service
    // receipt. Structural namespaces need no such receiver association; their
    // selected supply keys the exact requirement entry. All values are explicit.
    for adapter in &adapters {
        let adapter_rows::AdapterReceiver::Direct { namespace: owner } = adapter.receiver else {
            continue;
        };
        if !owner.is_valid() {
            continue;
        }
        let field = BoundaryField {
            symbol: owner,
            trait_symbol: adapter.receiver_trait,
            application: None,
            provider_plan_digest: None,
        };
        if !boundary_fields.contains(&field) {
            boundary_fields.push(field);
        }
    }

    // A borrowed nominal boundary-trait parameter uses the same selected
    // requirement as a boundary field. Its state-local parameter symbol, not
    // the repeated parameter spelling, identifies the receiver. This is not
    // routed Service erasure: generic and qualified carriers still require
    // the checked receipt below and must not enter through this narrow shape.
    for machine in typed
        .machines()
        .iter()
        .filter(|machine| product_roots.contains(machine.symbol))
    {
        for state in typed.machine_states(machine) {
            for parameter in typed.state_parameters(state) {
                let typed_trees::types::TypeReferenceNode::Reference { referee, .. } = typed
                    .type_reference_table
                    .type_reference(parameter.type_reference)
                else {
                    continue;
                };
                let Some(trait_symbol) = named_type_symbol(typed, *referee) else {
                    continue;
                };
                if parameter.is_const
                    || !typed.traits().iter().any(|definition| {
                        definition.symbol == trait_symbol && definition.is_boundary
                    })
                    || (!adapters
                        .iter()
                        .any(|adapter| adapter.receiver_trait == trait_symbol)
                        && !generic_requirements
                            .iter()
                            .any(|selected| selected.receiver_trait == trait_symbol))
                {
                    continue;
                }
                if !parameter.symbol.is_valid() {
                    diagnostics.push(Diagnostic::error(format!(
                        "borrowed boundary parameter `{}::{}` has no exact typed symbol for adapter dispatch",
                        machine.name, parameter.name,
                    )));
                    continue;
                }
                if let Some(existing) = boundary_fields
                    .iter()
                    .find(|field| field.symbol == parameter.symbol)
                {
                    if existing.trait_symbol != trait_symbol {
                        diagnostics.push(Diagnostic::error(format!(
                            "boundary receiver symbol {:?} maps to both trait symbols {:?} and {:?}",
                            parameter.symbol, existing.trait_symbol, trait_symbol,
                        )));
                    }
                    continue;
                }
                boundary_fields.push(BoundaryField {
                    symbol: parameter.symbol,
                    trait_symbol,
                    application: None,
                    provider_plan_digest: None,
                });
            }
        }
    }

    // Logical Binding parameter receipts identify the receiver independently
    // of provider selection. Only selected requirements contribute dispatch;
    // an unused helper may declare other requirements without installing them.
    let unit = &checked.facts.flow.terminal_unit_effects;
    let parameter_rosters = unit
        .machines
        .iter()
        .filter(|machine| product_roots.contains(machine.machine))
        .map(|machine| machine.structural_parameters.as_slice())
        .chain(
            unit.composed_machines
                .iter()
                .filter(|machine| product_roots.contains(machine.machine))
                .flat_map(|machine| {
                    machine
                        .states
                        .iter()
                        .map(|state| state.structural_parameters.as_slice())
                }),
        );
    for parameters in parameter_rosters {
        for parameter in parameters {
            let Some(receipt) = &parameter.binding_carrier else {
                continue;
            };
            let Some(source_parameter) = typed
                .machines()
                .iter()
                .flat_map(|machine| typed.machine_states(machine))
                .flat_map(|state| typed.state_parameters(state))
                .find(|source| source.symbol == receipt.source_parameter)
            else {
                continue;
            };
            let Ok(Some(carrier)) = typed_trees::service::classify_exact_service_parameter_carrier(
                typed,
                typed_trees::service::ServiceCarrierOccurrence {
                    type_reference: source_parameter.type_reference,
                    lexical_owner: source_parameter.symbol,
                    enclosing_data_applications: &[],
                },
            ) else {
                continue;
            };
            if carrier.carrier.requirement != receipt.requirement
                || typed.normalized_type_identity(source_parameter.type_reference)
                    != receipt.carrier_type_identity
            {
                continue;
            }
            let Some(application) =
                carrier
                    .carrier
                    .application(typed, receipt.source_parameter, &[])
            else {
                continue;
            };
            let Some(plan) = selected_service_plan_for_application(
                typed,
                calling_plans,
                selected_plans,
                &application,
                carrier.carrier.arguments(typed),
            ) else {
                continue;
            };
            let receiver = BoundaryField {
                symbol: receipt.source_parameter,
                trait_symbol: receipt.requirement,
                application: Some(application.environment()),
                provider_plan_digest: Some(*plan.identity_digest().as_bytes()),
            };
            if !adapters
                .iter()
                .any(|adapter| receiver.supplies_adapter(typed, adapter))
                && !generic_requirements.iter().any(|selected| {
                    receiver.supplies_requirement(
                        typed,
                        selected.provider_plan_digest,
                        selected.receiver_trait,
                        selected.requirement_symbol,
                    )
                })
            {
                continue;
            }
            if let Some(existing) = boundary_fields
                .iter()
                .find(|existing| existing.symbol == receipt.source_parameter)
            {
                if *existing != receiver {
                    diagnostics.push(Diagnostic::error(format!(
                        "boundary receiver symbol {:?} maps to both trait symbols {:?} and {:?}",
                        receipt.source_parameter, existing.trait_symbol, receipt.requirement,
                    )));
                }
                continue;
            }
            boundary_fields.push(receiver);
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    // Check every source occurrence against the exact selected requirement.
    // Source names remain diagnostics, never dispatch identity. Ambiguous
    // selected adapters must reject even when the result is not retained.
    for machine in typed
        .machines()
        .iter()
        .filter(|machine| product_roots.contains(machine.symbol))
    {
        for state in typed.machine_states(machine) {
            for statement in typed.statement_table.statements(state.statement_nodes) {
                if let Some(call) = statement.as_call() {
                    resolve_adapter_call(
                        typed,
                        &adapters,
                        &generic_requirements,
                        &boundary_fields,
                        call.receiver_symbol,
                        call.target_symbol,
                        call.target.as_str(),
                        &call.machine_arguments,
                    )
                    .map_err(|error| vec![error])?;
                }
            }
        }
    }
    for (handle, expression) in typed.expression_table.expression_entries() {
        if !product_references.expressions.contains(&handle) {
            continue;
        }
        let typed_trees::expression::ExpressionNode::Call(call) = expression else {
            continue;
        };
        let receiver = match typed.expression_table.expression(call.receiver) {
            typed_trees::expression::ExpressionNode::Member(member) => member.member_symbol,
            typed_trees::expression::ExpressionNode::Name(path) => path.symbol,
            _ => continue,
        };
        resolve_adapter_call(
            typed,
            &adapters,
            &generic_requirements,
            &boundary_fields,
            receiver,
            call.target_symbol,
            call.target.as_str(),
            &call.machine_arguments,
        )
        .map_err(|error| vec![error])?;
    }
    let mut dispatch = Vec::new();
    for receiver in &boundary_fields {
        for adapter in adapters
            .iter()
            .filter(|adapter| receiver.supplies_adapter(typed, adapter))
        {
            dispatch.push(checked_trees::CheckedBoundaryAdapterDispatch {
                receiver: receiver.symbol,
                requirement: adapter.requirement_symbol,
                realization_state: adapter.symbol,
                family_tuple: adapter.family_tuple.clone(),
            });
        }
    }
    // A `reaches`-bound call names the requirement's declaring trait
    // application as its receiver — `Service`, or its applied form such as
    // `Parent<u64>` — and carries no boundary field, so field-keyed emission
    // above can never claim its selected adapter. The settled row is still
    // the exact realization record; emit it keyed by that same declaring
    // trait, with the requirement normalized to its family template exactly
    // as call-side lookup reduces a closed application before comparing.
    for adapter in adapters.iter().filter(|adapter| {
        !boundary_fields
            .iter()
            .any(|receiver| receiver.supplies_adapter(typed, adapter))
    }) {
        let application = typed.closed_requirement_application(adapter.requirement_symbol);
        let receiver = application
            .map(|application| application.declaring_trait)
            .filter(|trait_symbol| trait_symbol.is_valid())
            .unwrap_or(adapter.receiver_trait);
        let requirement = application
            .map_or(adapter.requirement_symbol, |application| {
                application.template
            });
        dispatch.push(checked_trees::CheckedBoundaryAdapterDispatch {
            receiver,
            requirement,
            realization_state: adapter.symbol,
            family_tuple: adapter.family_tuple.clone(),
        });
    }
    Ok(dispatch)
}
