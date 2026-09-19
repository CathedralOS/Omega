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
mod boundary_fields;
mod signature_families;
#[cfg(test)]
mod tests;

use crate::boundary_dispatch::adapter_rows::{
    AdapterRow, GenericBoundaryRequirement, ResolvedAdapterRow, SelectedAdapterTarget,
    resolve_selected_adapter_row, resolve_selected_adapter_target,
};
use crate::boundary_dispatch::boundary_fields::{
    BoundaryField, BoundaryFieldDeclaration, exact_conformance_requirement_identity,
    named_type_symbol, resolve_adapter_call,
};
use crate::boundary_dispatch::signature_families::{FamilyProbe, finite_signature_family};
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use std::sync::Arc;
use typed_trees::TypedTrees;

/// The finite-family specialization demands one selected boundary surface
/// commits to before checking: every checked-adapter row whose requirement
/// declares a complete finite roster names its exact signature and provider
/// template here, so the family generator can materialize every roster
/// tuple's checked body instead of the settle boundary discovering a tuple
/// only when a static call site happened to demand it.
///
/// Demand collection applies the same resolution gates settlement does and
/// deliberately returns no demand for rows that cannot settle — non-finite
/// or open rosters, provider templates that cannot close the requirement's
/// value binders, adapters without a checked body, and rows that resolve to
/// no adapter at all. Those requirements keep their per-requirement
/// ineligibility at settlement; a demand only ever asks checking to supply
/// the roster the one roster authority already declared.
pub fn selected_boundary_family_specializations(
    typed: &TypedTrees,
    selected_plans: &effects::SelectedProviderPlanFacts,
) -> Vec<typed_trees_to_checked_trees::SelectedBoundaryFamilySpecialization> {
    let mut demands = Vec::new();
    for plan in selected_plans.plans() {
        for row in &plan.rows {
            // A row that cannot resolve fails identically at settlement,
            // where the same resolution runs authoritatively.
            let Ok(Some(SelectedAdapterTarget::TraitRequirement {
                method,
                requirement_owner,
                signature,
                ..
            })) = resolve_selected_adapter_target(typed, plan, row)
            else {
                continue;
            };
            if typed.state_signature_type_parameters(signature).is_empty() {
                continue;
            }
            let FamilyProbe::Finite { arity, .. } = finite_signature_family(typed, signature)
            else {
                continue;
            };
            if plan.provider_type.is_empty() {
                continue;
            }
            let Ok(adapter) = provider_planning::exact_checked_adapter(typed, plan, row) else {
                continue;
            };
            if adapter.attached_data.as_ref().map(|owner| owner.as_str())
                != Some(plan.provider_type.as_str())
                || !adapter.supply_mode.is_checked_body()
            {
                continue;
            }
            let provider_parameters = typed.machine_type_parameters(adapter);
            let provider_value_parameters = provider_parameters
                .iter()
                .filter(|parameter| {
                    matches!(
                        parameter.kind,
                        typed_trees::data::TypeParameterKind::Const { .. }
                            | typed_trees::data::TypeParameterKind::Value { .. }
                    )
                })
                .count();
            if provider_parameters.len() != provider_value_parameters
                || provider_value_parameters != arity
            {
                continue;
            }
            // The demand names the realized conformance edge itself: only an
            // adapter satisfying this exact overload may cover its roster.
            if !typed
                .machine_trait_conformances(adapter)
                .iter()
                .any(|conformance| {
                    conformance.external_binding.is_none()
                        && conformance.symbol == requirement_owner.symbol
                        && conformance
                            .requirement
                            .as_ref()
                            .is_some_and(|requirement| requirement.as_str() == method.name)
                        && exact_conformance_requirement_identity(
                            typed,
                            adapter,
                            requirement_owner,
                            method.name.as_str(),
                        )
                        .as_deref()
                            == Some(method.requirement_identity.as_str())
                })
            {
                continue;
            }
            let demand = typed_trees_to_checked_trees::SelectedBoundaryFamilySpecialization {
                requirement_signature: signature.symbol,
                realization_machine: adapter.symbol,
            };
            if !demands.contains(&demand) {
                demands.push(demand);
            }
        }
    }
    demands
}

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

fn plan_selected_boundary_adapter_dispatch(
    checked: &CheckedTrees,
    selected_plans: &effects::SelectedProviderPlanFacts,
) -> Result<Vec<checked_trees::CheckedBoundaryAdapterDispatch>, Vec<Diagnostic>> {
    let typed = &checked.typed;
    let mut adapters = Vec::new();
    let mut generic_requirements: Vec<GenericBoundaryRequirement> = Vec::new();
    let mut diagnostics = Vec::new();
    for plan in selected_plans.plans() {
        for row in &plan.rows {
            match resolve_selected_adapter_row(typed, plan, row) {
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
    let mut boundary_field_declarations = Vec::new();
    for data in typed.data_definitions() {
        for member in typed.data_members(data) {
            let typed_trees::data::DataMember::Field(field) = member else {
                continue;
            };
            let symbol = if let Some(requirement) =
                typed_trees::service::exact_bound_service_requirement(typed, field.type_reference)
            {
                let Some(authorization) = typed.fused_service_erasure(requirement) else {
                    continue;
                };
                if !adapters.iter().any(|adapter| {
                    adapter.receiver_trait == requirement
                        && adapter.provider_plan_digest == authorization.provider_plan_digest
                }) && !generic_requirements.iter().any(|selected| {
                    selected.receiver_trait == requirement
                        && selected.provider_plan_digest == authorization.provider_plan_digest
                }) {
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
            if let Some(existing) = boundary_field_declarations
                .iter()
                .find(|existing: &&BoundaryFieldDeclaration| existing.field.symbol == field.symbol)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "boundary field symbol {:?} maps to both trait symbols {:?} and {:?}",
                    field.symbol, existing.field.trait_symbol, symbol,
                )));
                continue;
            }
            boundary_field_declarations.push(BoundaryFieldDeclaration {
                owner: data.symbol,
                owner_name: data.name.as_str().to_owned(),
                field_name: field.name.as_str().to_owned(),
                field: BoundaryField {
                    symbol: field.symbol,
                    trait_symbol: symbol,
                },
            });
        }
    }

    // A top-level requirement row is reached by the direct call
    // `Owner::name(...)`, whose retained receiver is the exact nominal owner.
    // The owner joins the requirement symbol directly; no field, parameter or
    // routed-service receipt participates.
    let mut boundary_fields = Vec::new();
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

    // A direct `self.<field>` occurrence is stamped with the exact inherited
    // FIELD child of the attached machine, not the DATA declaration's field
    // symbol. Retain both coordinates: the declaration symbol serves nested
    // receiver leaves, while every exact machine-owned inherited symbol serves
    // direct statement and value receivers. Spelling is used only inside the
    // already-exact machine owner to resolve its unique child.
    boundary_fields.extend(
        boundary_field_declarations
            .iter()
            .map(|declaration| declaration.field),
    );
    for machine in typed.machines() {
        let Some(attached_data) = machine.attached_data.as_ref() else {
            continue;
        };
        let owners = typed
            .data_definitions()
            .iter()
            .filter(|data| data.name == *attached_data)
            .collect::<Vec<_>>();
        let [owner] = owners.as_slice() else {
            if boundary_field_declarations
                .iter()
                .any(|declaration| declaration.owner_name == attached_data.as_str())
            {
                diagnostics.push(Diagnostic::error(format!(
                    "adapter-dispatch machine `{}` resolves attached data `{attached_data}` to {} exact definitions",
                    machine.name,
                    owners.len(),
                )));
            }
            continue;
        };
        for declaration in boundary_field_declarations
            .iter()
            .filter(|declaration| declaration.owner == owner.symbol)
        {
            let symbols = machine
                .symbol
                .is_valid()
                .then(|| typed.symbols.child_handles(machine.symbol))
                .flatten()
                .into_iter()
                .flatten()
                .filter(|symbol| {
                    typed.symbols.get(*symbol).kind == symbols::SymbolKind::Field
                        && typed.symbols.name(*symbol) == declaration.field_name
                })
                .collect::<Vec<_>>();
            let [symbol] = symbols.as_slice() else {
                diagnostics.push(Diagnostic::error(format!(
                    "adapter-dispatch machine `{}` resolves inherited boundary field `{}::{}` to {} exact FIELD children",
                    machine.name,
                    declaration.owner_name,
                    declaration.field_name,
                    symbols.len(),
                )));
                continue;
            };
            boundary_fields.push(BoundaryField {
                symbol: *symbol,
                trait_symbol: declaration.field.trait_symbol,
            });
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
                if parameter.is_self
                    || parameter.is_const
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

    // The first routed-Service parameter rung is admitted only when checked Unit planning
    // retained an exact typed-parameter symbol plus the matching Fused plan
    // digest. Do not rediscover arbitrary Service-looking parameters from
    // names or types here: receipt custody is what authorizes receiver
    // erasure and direct adapter dispatch.
    for machine in &checked.facts.flow.terminal_unit_effects.machines {
        for parameter in &machine.structural_parameters {
            let Some(receipt) = &parameter.fused_service_erasure else {
                continue;
            };
            if !adapters.iter().any(|adapter| {
                adapter.receiver_trait == receipt.requirement
                    && adapter.provider_plan_digest == receipt.provider_plan_digest
            }) && !generic_requirements.iter().any(|selected| {
                selected.receiver_trait == receipt.requirement
                    && selected.provider_plan_digest == receipt.provider_plan_digest
            }) {
                diagnostics.push(Diagnostic::error(format!(
                    "routed service parameter {:?} has no exact Fused selected-provider-plan join",
                    receipt.source_parameter,
                )));
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
    // Source names remain diagnostics, never dispatch identity.
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
                forward_receiver: adapter.forward_receiver,
                family_tuple: adapter.family_tuple.clone(),
            });
        }
    }
    Ok(dispatch)
}

/// Whether a machine is a top-level `boundary requirement` a direct call may
/// execute through a settled dispatch row: public, nongeneric, and without a
/// `self` receiver.
pub(crate) fn is_directly_callable_top_level_requirement(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> bool {
    machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
        && machine.is_public
        && !machine.body_is_present
        && machine.lifetime_parameters.is_empty()
        && typed.machine_type_parameters(machine).is_empty()
        && matches!(
            typed.machine_states(machine),
            [entry] if !typed.state_parameters(entry).iter().any(|parameter| parameter.is_self)
        )
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
        // intrinsic bridge (the requirement use is stamped with the plan and
        // rewritten at execution settlement); any other external binding has
        // no direct-call execution route yet and says so.
        let external = selected_plans
            .plans()
            .iter()
            .filter(|plan| {
                provider_planning::service_schema::schema_binds_exact_boundary_requirement(
                    typed,
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
            ) {
                return;
            }
            diagnostics.push(Diagnostic::error(format!(
                "direct call `{target_name}` names public boundary requirement `{}`, whose selected provider `{}` realizes it through external binding {:?} rather than a checked machine body or compiler intrinsic; the direct-call route executes only those",
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

/// Whether settlement retained a direct-call dispatch row for a top-level
/// boundary requirement: its `requirement` is the entry state of a
/// `TopLevelRequirement` machine rather than a trait signature.
pub(crate) fn has_top_level_requirement_dispatch(checked: &CheckedTrees) -> bool {
    checked.facts.boundary_adapter_dispatch.iter().any(|row| {
        checked.typed.machines().iter().any(|machine| {
            machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
                && checked
                    .typed
                    .machine_states(machine)
                    .first()
                    .is_some_and(|entry| entry.symbol == row.requirement)
        })
    })
}
