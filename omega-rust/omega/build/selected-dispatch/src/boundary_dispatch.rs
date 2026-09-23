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
            if typed.attached_data_path(adapter).as_deref() != Some(plan.provider_type.as_str())
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
    provider_plan_digest: &[u8; 32],
) -> bool {
    let Some(plan) = selected_plans
        .plans()
        .iter()
        .find(|plan| plan.identity_digest().as_bytes() == provider_plan_digest)
    else {
        return false;
    };
    typed.traits().iter().any(|definition| {
        definition.symbol == requirement
            && provider_planning::service_schema::from_typed(typed, definition)
                .is_some_and(|schema| schema == plan.schema)
    })
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
                if !selected_service_plan_matches_requirement(
                    typed,
                    selected_plans,
                    requirement,
                    &authorization.provider_plan_digest,
                ) {
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
    // routed-service receipt participates. A `self` requirement's calls keep
    // the receiver place instead, so the owner key is registered only for a
    // receiver-free row.
    let mut boundary_fields = Vec::new();
    for adapter in &adapters {
        if adapter.forward_receiver {
            continue;
        }
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

    // A `self`/`&self`/`&mut self` requirement is called through a member receiver
    // (`token.consume()` or a projected place like `holder.inner.token.consume()`),
    // whose retained receiver symbol is the receiver PLACE leaf -- a per-site
    // parameter, `self` binding, local, or projected field member -- not the
    // nominal owner. Register the receiver place of each member call that
    // targets a settled self row after verifying its declared type is the
    // requirement owner; the row's forward_receiver flag carries that place
    // into the adapter's leading argument. A borrowed `&self`/`&mut self`
    // requirement registers the same place: the member call borrows the place
    // for the call, and the rewrite splices `&place`/`&mut place` as the
    // adapter's leading argument of the same access. Write-only and qualified
    // receivers do not reach here: settlement already rejected a `&write
    // self`/qualified requirement row, and a receiver place that does not
    // resolve to the owner keeps no field, so its call is diagnosed below.
    let self_adapters = adapters
        .iter()
        .filter(|adapter| adapter.forward_receiver && adapter.top_level_owner.is_some())
        .collect::<Vec<_>>();
    if !self_adapters.is_empty() {
        // The declared types of the member `member_name` inside a named data
        // type: the owning definition is already exact, so the member name
        // resolves inside it alone.
        let member_field_types = |type_reference, member_name: &str| {
            let mut type_references = Vec::new();
            if let Some(data_symbol) = named_type_symbol(typed, type_reference)
                && let Some(owner) = typed
                    .data_definitions()
                    .iter()
                    .find(|data| data.symbol == data_symbol)
            {
                for member in typed.data_members(owner) {
                    if let typed_trees::data::DataMember::Field(field) = member
                        && field.name.as_str() == member_name
                    {
                        type_references.push(field.type_reference);
                    }
                }
            }
            type_references
        };
        // The declared type of one receiver place path, wherever it is
        // bound: the root place resolves through its parameter or local
        // declaration -- place symbols are unique across the program, so the
        // search is exact rather than scoped to the call's state -- then each
        // projected member resolves its declared field type inside the
        // previous member's named data definition.
        let receiver_place_types =
            |root: symbols::SymbolHandle, members: &[typed_trees::name::Identifier]| {
                let mut type_references = Vec::new();
                // A `self.`-rooted receiver names its enclosing machine or
                // state as the root place, whose type is the machine's
                // attached data rather than a parameter or local
                // declaration.
                let mut self_attached_data = Vec::new();
                for machine in typed.machines() {
                    if machine.symbol == root
                        || typed
                            .machine_states(machine)
                            .iter()
                            .any(|state| state.symbol == root)
                    {
                        self_attached_data.push(machine.attached_data_symbol);
                    }
                    for state in typed.machine_states(machine) {
                        for parameter in typed.state_parameters(state) {
                            if parameter.symbol == root {
                                type_references.push(parameter.type_reference);
                            }
                        }
                        for statement in typed.statement_table.statements(state.statement_nodes) {
                            if let typed_trees::statement::StatementNode::LocalData(local) =
                                statement
                                && local.symbol == root
                            {
                                type_references.push(local.type_reference);
                            }
                        }
                    }
                }
                for (member_index, member) in members.iter().enumerate() {
                    let mut next = type_references
                        .iter()
                        .flat_map(|type_reference| {
                            member_field_types(*type_reference, member.as_str())
                        })
                        .collect::<Vec<_>>();
                    if member_index == 0 {
                        for data_symbol in self_attached_data.iter().copied() {
                            if let Some(owner) = typed
                                .data_definitions()
                                .iter()
                                .find(|data| data.symbol == data_symbol)
                            {
                                next.extend(typed.data_members(owner).iter().filter_map(
                                    |data_member| match data_member {
                                        typed_trees::data::DataMember::Field(field)
                                            if field.name.as_str() == member.as_str() =>
                                        {
                                            Some(field.type_reference)
                                        }
                                        _ => None,
                                    },
                                ));
                            }
                        }
                    }
                    type_references = next;
                    if type_references.is_empty() {
                        break;
                    }
                }
                type_references
            };
        let mut register_receiver_place =
            |root: symbols::SymbolHandle,
             members: &[typed_trees::name::Identifier],
             leaf: symbols::SymbolHandle,
             target_symbol: symbols::SymbolHandle| {
                let Some(adapter) = self_adapters
                    .iter()
                    .find(|adapter| adapter.requirement_symbol == target_symbol)
                else {
                    return;
                };
                let Some(owner) = adapter.top_level_owner else {
                    return;
                };
                if !receiver_place_types(root, members)
                    .iter()
                    .any(|type_reference| named_type_symbol(typed, *type_reference) == Some(owner))
                {
                    return;
                }
                let field = BoundaryField {
                    symbol: leaf,
                    trait_symbol: adapter.receiver_trait,
                };
                if !boundary_fields.contains(&field) {
                    boundary_fields.push(field);
                }
            };
        for machine in typed.machines() {
            for state in typed.machine_states(machine) {
                for statement in typed.statement_table.statements(state.statement_nodes) {
                    let typed_trees::statement::StatementNode::Call(call) = statement else {
                        continue;
                    };
                    let members = typed.statement_table.name_path_members(call.receiver);
                    if members.is_empty()
                        || !call.receiver_root_symbol.is_valid()
                        || !call.receiver_symbol.is_valid()
                    {
                        continue;
                    }
                    register_receiver_place(
                        call.receiver_root_symbol,
                        &members[1..],
                        call.receiver_symbol,
                        call.target_symbol,
                    );
                }
            }
        }
        for (_, expression) in typed.expression_table.expression_entries() {
            let typed_trees::expression::ExpressionNode::Call(call) = expression else {
                continue;
            };
            // The receiver expression is a name path or a member-projection
            // chain; walk it to the root place, collecting member names in
            // path order and the leaf's exact member symbol.
            let leaf = match typed.expression_table.expression(call.receiver) {
                typed_trees::expression::ExpressionNode::Member(member) => member.member_symbol,
                typed_trees::expression::ExpressionNode::Name(path) => path.symbol,
                _ => continue,
            };
            let mut members = Vec::new();
            let mut cursor = call.receiver;
            let root = loop {
                match typed.expression_table.expression(cursor) {
                    typed_trees::expression::ExpressionNode::Member(member) => {
                        members.push(member.member.clone());
                        cursor = member.receiver;
                    }
                    typed_trees::expression::ExpressionNode::Name(path) => break path.symbol,
                    _ => break symbols::SymbolHandle::invalid(),
                }
            };
            if !root.is_valid() || !leaf.is_valid() {
                continue;
            }
            members.reverse();
            register_receiver_place(root, &members, leaf, call.target_symbol);
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
            .filter(|data| data.symbol == machine.attached_data_symbol)
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
            if !selected_service_plan_matches_requirement(
                typed,
                selected_plans,
                receipt.requirement,
                &receipt.provider_plan_digest,
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
    // Source names remain diagnostics, never dispatch identity. A member call
    // whose receiver place failed to join a settled receiver-forwarding row
    // must not silently keep the requirement seam: it would interpret as an
    // unselected reach row while a provider was selected for it.
    let receiver_forwarded_target = |target_symbol| {
        adapters.iter().any(|adapter| {
            adapter.forward_receiver
                && adapter.top_level_owner.is_some()
                && adapter.requirement_symbol == target_symbol
        })
    };
    for machine in typed.machines() {
        for state in typed.machine_states(machine) {
            for statement in typed.statement_table.statements(state.statement_nodes) {
                if let typed_trees::statement::StatementNode::Call(call) = statement {
                    let adapter = resolve_adapter_call(
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
                    if adapter.is_none() && receiver_forwarded_target(call.target_symbol) {
                        return Err(vec![Diagnostic::error(format!(
                            "member call `{}` names a selected receiver-bearing boundary requirement, but its receiver place does not join the settled row: only an owned receiver place of the requirement owner type forwards as argument 0",
                            call.target.as_str(),
                        ))]);
                    }
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
        let adapter = resolve_adapter_call(
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
        if adapter.is_none() && receiver_forwarded_target(call.target_symbol) {
            return Err(vec![Diagnostic::error(format!(
                "member call `{}` names a selected receiver-bearing boundary requirement, but its receiver place does not join the settled row: only an owned receiver place of the requirement owner type forwards as argument 0",
                call.target.as_str(),
            ))]);
        }
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
        // rewritten at execution settlement); an evaluated import or syscall
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
