//! Resolve checked boundary calls to exact selected adapters without rewriting
//! their source meaning. Exact and receiver-forwarding adapters share the same
//! association; execution consumes it, while Terminal retains the requirement.
//!
//! This file settles and plans selected boundary adapter dispatch.
//! `adapter_rows.rs` carries adapter rows and their resolution,
//! `signature_families.rs` probes finite signature families and
//! `boundary_fields.rs` names boundary fields, traits, receivers and
//! adapter calls; `tests.rs` holds the dispatch tests.

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
use std::sync::Arc;
use typed_trees::TypedTrees;

/// Bind selected execution without changing typed source or source-derived plans.
/// All fallible work completes before publishing the association set.
pub fn settle_selected_boundary_adapter_dispatch(
    checked: &mut Arc<CheckedTrees>,
    selected_plans: &effects::SelectedProviderPlanFacts,
) -> Result<(), Vec<Diagnostic>> {
    let dispatch = plan_selected_boundary_adapter_dispatch(checked, selected_plans)?;
    if checked.facts.boundary_adapter_dispatch != dispatch {
        Arc::make_mut(checked).facts.boundary_adapter_dispatch = dispatch;
    }
    Ok(())
}

/// Service erasure joins the selected plan, not its checked-adapter subset.
/// Intrinsic and foreign rows deliberately produce no adapter dispatch; an
/// unrelated checked provider must not make their service carriers invalid.
/// Exact schema ownership still matters: another requirement's valid digest
/// cannot authorize this receiver. Fused selection provenance and carrier
/// custody remain independently checked by `service_custody` before native use.
fn selected_service_plan_matches_requirement(
    typed: &TypedTrees,
    selected_plans: &effects::SelectedProviderPlanFacts,
    requirement: symbols::SymbolHandle,
) -> bool {
    let Some(schema) = typed
        .traits()
        .iter()
        .find(|definition| definition.symbol == requirement)
        .and_then(|definition| {
            provider_planning::service_schema::from_typed(
                provider_planning::ProviderPlanningContext::new(
                    typed,
                    source::DependencyScope::Product,
                ),
                definition,
            )
        })
    else {
        return false;
    };
    selected_plans
        .plans()
        .iter()
        .filter(|plan| plan.schema == schema)
        .count()
        == 1
}

fn plan_selected_boundary_adapter_dispatch(
    checked: &CheckedTrees,
    selected_plans: &effects::SelectedProviderPlanFacts,
) -> Result<Vec<checked_trees::CheckedBoundaryAdapterDispatch>, Vec<Diagnostic>> {
    // Review retains checked Build bodies, but this operation realizes only
    // Product execution. Build's admitted provider context is independent:
    // requiring its calls to join Product plans would replay the wrong purpose.
    // Select all Product roots, not a guessed reachable subset or callee names.
    let context = provider_planning::ProviderPlanningContext::new(
        &checked.typed,
        source::DependencyScope::Product,
    );
    let product_roots = typed_trees::references::DeclarationSelection::from_symbols(
        typed_trees::references::collect_program_references(&checked.typed)
            .declarations
            .into_iter()
            .filter(|symbol| context.contains(*symbol)),
    );
    let product =
        typed_trees_to_checked_trees::project_selected_program(&checked.typed, &product_roots);
    let typed = &product;
    let mut adapters = Vec::new();
    let mut generic_requirements: Vec<GenericBoundaryRequirement> = Vec::new();
    let mut diagnostics = Vec::new();
    for plan in selected_plans.plans() {
        for row in &plan.rows {
            match resolve_selected_adapter_row(
                typed,
                plan,
                row,
                selected_plans.specialized_requirement_rows(),
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
    // A direct call to a public receiver-free top-level requirement is
    // executable only through a settled row: without a selected provider the
    // call rejects here, before either engine could reach the bodyless
    // declaration. Receiver-bearing requirements keep their existing routes.
    reject_unselected_direct_requirement_calls(typed, &adapters, selected_plans, &mut diagnostics);
    if adapters.is_empty() && generic_requirements.is_empty() {
        return diagnostics.is_empty().then(Vec::new).ok_or(diagnostics);
    }

    // Exact typed field symbol -> exact boundary-trait symbol. Field spellings
    // may repeat across data owners and never participate in dispatch.
    let mut boundary_fields = Vec::new();
    for data in typed.data_definitions() {
        for member in typed.data_members(data) {
            let typed_trees::data::DataMember::Field(field) = member else {
                continue;
            };
            let symbol = if let Some(requirement) =
                typed_trees::service::exact_bound_service_requirement(typed, field.type_reference)
            {
                if typed.fused_service_erasure(requirement).is_none() {
                    continue;
                }
                if !selected_service_plan_matches_requirement(typed, selected_plans, requirement) {
                    diagnostics.push(Diagnostic::error(format!(
                        "routed service field `{}::{}` has no exact Fused selected-provider-plan join",
                        data.name, field.name,
                    )));
                    continue;
                }
                requirement
            } else {
                let typed_trees::types::TypeReferenceNode::Named { symbol, .. } = typed
                    .type_reference_table
                    .type_reference(field.type_reference)
                else {
                    continue;
                };
                *symbol
            };
            if !adapters
                .iter()
                .any(|adapter| adapter.receiver_trait == symbol)
                && !generic_requirements
                    .iter()
                    .any(|selected| selected.receiver_trait == symbol)
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
            boundary_fields.push(BoundaryField {
                symbol: field.symbol,
                trait_symbol: symbol,
            });
        }
    }

    // A top-level requirement row is reached by the direct call
    // `Owner::name(...)`, whose retained receiver is the exact nominal owner.
    // The owner joins the requirement symbol directly; no field, parameter or
    // routed-service receipt participates; all value arguments remain explicit.
    for adapter in &adapters {
        let Some(owner) = adapter.top_level_owner else {
            continue;
        };
        let field = BoundaryField {
            symbol: owner,
            trait_symbol: adapter.receiver_trait,
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
    for machine in typed.machines() {
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
                });
            }
        }
    }

    // Routed Binding parameters are admitted only when checked Unit planning
    // retained an exact typed-parameter symbol plus the matching Fused plan
    // digest. Do not rediscover arbitrary Service-looking parameters from
    // names or types here: receipt custody is what authorizes receiver
    // erasure and direct adapter dispatch.
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
            let Some(receipt) = &parameter.fused_service_erasure else {
                continue;
            };
            if !selected_service_plan_matches_requirement(
                typed,
                selected_plans,
                receipt.requirement,
            ) {
                diagnostics.push(Diagnostic::error(format!(
                    "routed service parameter {:?} has no exact Fused selected-provider-plan join",
                    receipt.source_parameter,
                )));
                continue;
            }
            if !adapters
                .iter()
                .any(|adapter| adapter.receiver_trait == receipt.requirement)
                && !generic_requirements
                    .iter()
                    .any(|selected| selected.receiver_trait == receipt.requirement)
            {
                continue;
            }
            if let Some(existing) = boundary_fields
                .iter()
                .find(|existing| existing.symbol == receipt.source_parameter)
            {
                if existing.trait_symbol != receipt.requirement {
                    diagnostics.push(Diagnostic::error(format!(
                        "boundary receiver symbol {:?} maps to both trait symbols {:?} and {:?}",
                        receipt.source_parameter, existing.trait_symbol, receipt.requirement,
                    )));
                }
                continue;
            }
            boundary_fields.push(BoundaryField {
                symbol: receipt.source_parameter,
                trait_symbol: receipt.requirement,
            });
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    // Check every source occurrence against the exact selected requirement.
    // Source names remain diagnostics, never dispatch identity. Ambiguous
    // selected adapters must reject even when the result is not retained.
    for machine in typed.machines() {
        for state in typed.machine_states(machine) {
            for statement in typed.statement_table.statements(state.statement_nodes) {
                if let typed_trees::statement::StatementNode::Call(call) = statement {
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
    for (_, expression) in typed.expression_table.expression_entries() {
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
    for receiver in boundary_fields {
        for adapter in adapters
            .iter()
            .filter(|adapter| adapter.receiver_trait == receiver.trait_symbol)
        {
            dispatch.push(checked_trees::CheckedBoundaryAdapterDispatch {
                receiver: receiver.symbol,
                requirement: adapter.requirement_symbol,
                realization_state: adapter.symbol,
                forward_receiver: false,
                family_tuple: adapter.family_tuple.clone(),
            });
        }
    }
    Ok(dispatch)
}

/// Whether a machine is a top-level `boundary requirement` a direct call may
/// execute through a settled dispatch row: public, free of static generic
/// binders, and without a `self` receiver. An erased lifetime telescope is
/// admitted: it contributes no static application arguments, so the settled
/// adapter row carries the call exactly as for a nongeneric requirement.
pub(crate) fn is_directly_callable_top_level_requirement(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> bool {
    machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
        && machine.is_public
        && !machine.body_is_present
        && typed.machine_type_parameters(machine).is_empty()
        && matches!(typed.machine_states(machine), [_])
}

fn reject_unselected_direct_requirement_calls(
    typed: &TypedTrees,
    adapters: &[AdapterRow],
    selected_plans: &effects::SelectedProviderPlanFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut reported = Vec::new();
    let mut check = |target_symbol: symbols::SymbolHandle, target_name: &str| {
        if !target_symbol.is_valid() || reported.contains(&target_symbol) {
            return;
        }
        // A direct machine call targets the entry state of its callee.
        let Some(requirement) = typed.machines().iter().find(|machine| {
            typed
                .machine_states(machine)
                .first()
                .is_some_and(|entry| entry.symbol == target_symbol)
        }) else {
            return;
        };
        if !is_directly_callable_top_level_requirement(typed, requirement)
            || adapters.iter().any(|adapter| {
                adapter.top_level_owner.is_some() && adapter.requirement_symbol == target_symbol
            })
        {
            return;
        }
        reported.push(target_symbol);
        // A selected compiler-intrinsic plan is executed by the named-float
        // intrinsic bridge (the requirement use joins the plan by requirement
        // identity and is rewritten at execution settlement); an evaluated import or syscall
        // binding keeps the call on the requirement's retained boundary seam,
        // which native realization joins to the normalized external-binding
        // row by requirement identity — the `via` leaf executes through that
        // route without an adapter rewrite. Other bindings have no
        // direct-call execution route and say so.
        let external = selected_plans
            .plans()
            .iter()
            .filter(|plan| {
                provider_planning::service_schema::schema_binds_exact_boundary_requirement(
                    provider_planning::ProviderPlanningContext::new(
                        typed,
                        source::DependencyScope::Product,
                    ),
                    &plan.schema,
                    requirement,
                )
            })
            .flat_map(|plan| plan.rows.iter().map(move |row| (plan, row)))
            .find(|(_, row)| {
                !matches!(
                    row.binding,
                    effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
                )
            });
        if let Some((plan, row)) = external {
            if matches!(
                row.binding,
                effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
                    | effects::provider_plan::ProviderBinding::Import { .. }
                    | effects::provider_plan::ProviderBinding::Syscall { .. }
            ) {
                return;
            }
            diagnostics.push(Diagnostic::error(format!(
                "direct call `{target_name}` names public boundary requirement `{}`, whose selected provider `{}` realizes it through external binding {:?} rather than a checked machine body, compiler intrinsic, or evaluated import/syscall binding; the direct-call route executes only those",
                requirement.name, plan.name, row.binding,
            )));
            return;
        }
        diagnostics.push(Diagnostic::error(format!(
            "direct call `{target_name}` names public boundary requirement `{}`, which has no selected provider; select an admitted provider whose checked machine `satisfies {}` before calling it",
            requirement.name, requirement.name,
        )));
    };
    for machine in typed.machines() {
        for state in typed.machine_states(machine) {
            for statement in typed.statement_table.statements(state.statement_nodes) {
                if let typed_trees::statement::StatementNode::Call(call) = statement {
                    check(call.target_symbol, call.target.as_str());
                }
            }
        }
    }
    for (_, expression) in typed.expression_table.expression_entries() {
        if let typed_trees::expression::ExpressionNode::Call(call) = expression {
            check(call.target_symbol, call.target.as_str());
        }
    }
}
