//! Adapter rows, generic boundary requirements and their resolution.

use crate::boundary_dispatch::boundary_fields::{
    exact_adapter_receiver_shape, exact_boundary_trait, exact_conformance_requirement_identity,
    named_type_symbol,
};
use crate::boundary_dispatch::signature_families::{FamilyProbe, finite_signature_family};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AdapterRow {
    pub(crate) receiver_trait: symbols::SymbolHandle,
    pub(crate) provider_plan_digest: [u8; 32],
    pub(crate) receiver_trait_name: String,
    pub(crate) requirement: String,
    pub(crate) requirement_identity: String,
    pub(crate) requirement_symbol: symbols::SymbolHandle,
    pub(crate) adapter_target: String,
    pub(crate) symbol: symbols::SymbolHandle,
    /// Self-forwarding shape: prepend the call's receiver as argument 0.
    pub(crate) forward_receiver: bool,
    /// Finite-family tuple this row realizes: canonical const identities in
    /// the requirement's value-binder declaration order (the same strings a
    /// `MachineSpecialization` retains in `const_argument_identities`). Empty
    /// on an exact nongeneric row, which matches every call to its
    /// requirement without consulting static arguments.
    pub(crate) family_tuple: Box<[String]>,
    /// Readable tuple spellings for diagnostics only; `family_tuple` is the
    /// semantic key.
    pub(crate) family_tuple_display: Box<[String]>,
    /// Set on a row realizing a top-level `boundary requirement`: the exact
    /// nominal owner (`Owner` in `Owner::name`) a direct call retains as its
    /// receiver symbol. `receiver_trait` is then the requirement symbol
    /// itself, never a boundary trait, and no field or parameter joins it.
    pub(crate) top_level_owner: Option<symbols::SymbolHandle>,
}

/// A selected requirement that declares local generic binders but cannot
/// produce dispatch rows: either it declares no complete finite `where`
/// family, or the selected provider's checked specializations do not cover
/// every declared tuple. Ineligibility is individual: the requirement
/// supplies no dispatch row, sibling requirements still settle, and a call
/// targeting it rejects below instead of silently dispatching every tuple to
/// the unbound generic template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GenericBoundaryRequirement {
    pub(crate) receiver_trait: symbols::SymbolHandle,
    pub(crate) provider_plan_digest: [u8; 32],
    pub(crate) requirement_symbol: symbols::SymbolHandle,
    pub(crate) requirement_identity: String,
    /// Why the family supplied no rows, retained for the call diagnostic.
    pub(crate) reason: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ResolvedAdapterRow {
    /// One exact checked realization of the requirement: nongeneric, or one
    /// tuple of a finite generic family.
    Adapter(AdapterRow),
    /// The requirement is dynamically ineligible for want of tuple rows.
    GenericRequirement(GenericBoundaryRequirement),
}

#[cfg(test)]
impl ResolvedAdapterRow {
    #[cfg(test)]
    pub(crate) fn expect_adapter(self) -> AdapterRow {
        match self {
            Self::Adapter(row) => row,
            Self::GenericRequirement(requirement) => panic!(
                "requirement `{}` is dynamically ineligible, not an exact adapter",
                requirement.requirement_identity
            ),
        }
    }
}

/// What one selected checked-adapter row's schema resolves to before tuple
/// coverage is consulted: either a top-level `boundary requirement`'s single
/// slot, or the exact signature on the requirement-owning boundary trait.
/// `None` means the row never reaches adapter dispatch — a non-adapter
/// binding, or a schema bound to an exact boundary operator.
pub(crate) enum SelectedAdapterTarget<'a> {
    TopLevelRequirement {
        method: &'a effects::provider_plan::ServiceMethod,
        requirement: &'a typed_trees::machine::Machine,
    },
    TraitRequirement {
        method: &'a effects::provider_plan::ServiceMethod,
        receiver_trait: &'a typed_trees::trait_definition::TraitDefinition,
        requirement_owner: &'a typed_trees::trait_definition::TraitDefinition,
        signature: &'a typed_trees::signature::StateSignature,
    },
}

/// The shared schema-resolution prefix of adapter-row settlement and
/// boundary family-demand collection. Running it twice on the same program
/// cannot diverge: demand derivation names exactly the signature settlement
/// would look up, and a row that fails here fails identically in both paths.
pub(crate) fn resolve_selected_adapter_target<'a>(
    typed: &'a TypedTrees,
    plan: &'a effects::provider_plan::ProviderPlan,
    row: &'a effects::provider_plan::ProviderPlanRow,
) -> Result<Option<SelectedAdapterTarget<'a>>, Diagnostic> {
    if !matches!(
        row.binding,
        effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
    ) {
        return Ok(None);
    }
    if typed.operators().iter().any(|operator| {
        provider_planning::service_schema::schema_binds_exact_boundary_operator(
            typed,
            &plan.schema,
            operator,
        )
    }) {
        return Ok(None);
    }
    if row.requirement_identity.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter row `{}` in ProviderPlan `{}` has no exact overload identity",
            row.method, plan.name,
        )));
    }
    let methods = plan
        .schema
        .methods
        .iter()
        .filter(|method| plan.schema.row_binds_method(row, method))
        .collect::<Vec<_>>();
    let [method] = methods.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter row `{}` / `{}` in ProviderPlan `{}` binds {} exact schema methods",
            row.method,
            row.requirement_identity,
            plan.name,
            methods.len(),
        )));
    };

    // A top-level `boundary requirement` is its own single-row slot: the
    // schema names the requirement machine, not a boundary trait.
    let top_level_requirements = typed
        .machines()
        .iter()
        .filter(|requirement| {
            provider_planning::service_schema::schema_binds_exact_boundary_requirement(
                typed,
                &plan.schema,
                requirement,
            )
        })
        .collect::<Vec<_>>();
    match top_level_requirements.as_slice() {
        [requirement] => {
            return Ok(Some(SelectedAdapterTarget::TopLevelRequirement {
                method,
                requirement,
            }));
        }
        [] => {}
        many => {
            return Err(Diagnostic::error(format!(
                "selected checked-adapter selected schema `{}` resolves to {} exact top-level boundary requirements",
                plan.schema.trait_name,
                many.len(),
            )));
        }
    }

    let receiver_trait = exact_boundary_trait(
        typed,
        &plan.schema.trait_name,
        plan.schema.trait_package_identity,
        "selected schema",
    )?;
    let requirement_owner = exact_boundary_trait(
        typed,
        &method.requirement_owner,
        method.requirement_owner_package_identity,
        "requirement owner",
    )?;
    let signatures = typed
        .trait_machine_signatures(requirement_owner)
        .iter()
        .filter(|signature| {
            signature.name.as_str() == method.name
                && typed
                    .normalized_trait_requirement_overload_identity(requirement_owner, signature)
                    .identity()
                    == method.requirement_identity
        })
        .collect::<Vec<_>>();
    let [signature] = signatures.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter requirement `{}` resolves to {} exact typed signatures",
            method.requirement_identity,
            signatures.len(),
        )));
    };
    if !signature.symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter requirement `{}` has no exact typed symbol",
            method.requirement_identity,
        )));
    }
    Ok(Some(SelectedAdapterTarget::TraitRequirement {
        method,
        receiver_trait,
        requirement_owner,
        signature,
    }))
}

pub(crate) fn resolve_selected_adapter_row(
    typed: &TypedTrees,
    plan: &effects::provider_plan::ProviderPlan,
    row: &effects::provider_plan::ProviderPlanRow,
) -> Result<Vec<ResolvedAdapterRow>, Diagnostic> {
    use effects::provider_plan::ProviderBinding;

    let ProviderBinding::CheckedAdapter {
        machine_identity, ..
    } = &row.binding
    else {
        return Ok(Vec::new());
    };
    let (method, receiver_trait, requirement_owner, signature) =
        match resolve_selected_adapter_target(typed, plan, row)? {
            Some(SelectedAdapterTarget::TopLevelRequirement {
                method,
                requirement,
            }) => {
                return resolve_top_level_requirement_adapter_row(
                    typed,
                    plan,
                    row,
                    method,
                    requirement,
                    machine_identity,
                );
            }
            Some(SelectedAdapterTarget::TraitRequirement {
                method,
                receiver_trait,
                requirement_owner,
                signature,
            }) => (method, receiver_trait, requirement_owner, signature),
            None => return Ok(Vec::new()),
        };

    // Requirement-local binders make this a family of rows keyed by canonical
    // value tuple, not one exact overload. The signature's `where` clause
    // must declare the complete finite roster explicitly.
    if !typed.state_signature_type_parameters(signature).is_empty() {
        return resolve_family_adapter_row(
            typed,
            plan,
            row,
            method,
            receiver_trait,
            requirement_owner,
            signature,
            machine_identity,
        );
    }

    if plan.provider_type.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter ProviderPlan `{}` has no nominal provider type",
            plan.name,
        )));
    }
    let adapter = provider_planning::exact_checked_adapter(typed, plan, row)?;
    if typed.attached_data_path(adapter).as_deref() != Some(plan.provider_type.as_str()) {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` does not belong to nominal provider `{}`",
            plan.provider_type,
        )));
    }
    if !adapter.supply_mode.is_checked_body() {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` is not a checked body",
        )));
    }
    // A generic machine cannot be the exact realization of a nongeneric
    // requirement: its unbound binders have no tuple to close them.
    if !typed.machine_type_parameters(adapter).is_empty() {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` is generic over {} machine binders; only an exact nongeneric realization can settle a boundary row",
            typed.machine_type_parameters(adapter).len(),
        )));
    }
    let Some(entry) = typed.machine_states(adapter).first() else {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` has no executable entry state",
        )));
    };
    if !entry.symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` has no exact entry-state symbol",
        )));
    }

    let conformances = typed
        .machine_trait_conformances(adapter)
        .iter()
        .filter(|conformance| {
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
        .count();
    if conformances != 1 {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` binds exact overload `{}` through {conformances} checked conformances",
            method.requirement_identity,
        )));
    }

    // ServiceMethod arity excludes the language receiver. A concrete
    // provider's `self` is therefore part of its exact realization, not the
    // explicit boundary binding that adapter dispatch may forward.
    let actual_parameters = typed
        .state_parameters(entry)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    // A trait requirement's receiver is never the adapter's leading
    // parameter: the adapter's own `self` belongs to the provider, so a
    // forwarded receiver here is always an owned `Owner` argument.
    let forward_receiver = match exact_adapter_receiver_shape(
        typed,
        &actual_parameters,
        method.parameter_count,
        requirement_owner.symbol,
        None,
    ) {
        Some(forward_receiver) => forward_receiver,
        None => {
            let count = actual_parameters.len();
            return Err(Diagnostic::error(format!(
                "selected checked adapter `{machine_identity}` has {count} non-self entry parameters; exact overload `{}` requires {} or one leading `{}` receiver",
                method.requirement_identity, method.parameter_count, requirement_owner.name,
            )));
        }
    };

    Ok(vec![ResolvedAdapterRow::Adapter(AdapterRow {
        receiver_trait: receiver_trait.symbol,
        provider_plan_digest: *plan.identity_digest().as_bytes(),
        receiver_trait_name: receiver_trait.name.as_str().to_owned(),
        requirement: method.name.clone(),
        requirement_identity: method.requirement_identity.clone(),
        requirement_symbol: signature.symbol,
        adapter_target: adapter.name.as_str().to_owned(),
        symbol: entry.symbol,
        forward_receiver,
        family_tuple: Box::default(),
        family_tuple_display: Box::default(),
        top_level_owner: None,
    })])
}

/// Resolve the one exact checked adapter realizing a top-level `boundary
/// requirement`. Two rungs are deliberately closed: a public requirement
/// carrying no static generic binders (`pub boundary requirement
/// Owner::name(...);`, optionally with an erased lifetime telescope
/// `Owner::name<'a>(...)`) and a checked-body adapter with no static generic
/// binders whose sole `satisfies` edge rejoins that exact requirement symbol
/// with the same non-self arity. The requirement's lifetime telescope is the
/// adapter's own telescope by the identity application conformance checked,
/// so an erased telescope needs no row argument of its own. A receiver-free
/// requirement admits only an exact-arity adapter and the row keys on the
/// nominal owner the direct call retains as its receiver. A by-value `self`
/// requirement (`Owner::name(self, ...)`) admits only an adapter taking the
/// owner as its leading parameter; the row forwards the member call's
/// receiver place as argument zero, so the row keys on each receiver place's
/// own symbol instead of the owner. A shared `&self` requirement
/// (`Owner::name(&self, ...)`) admits the same forwarding shape with the
/// adapter's leading parameter declared `&Owner`; the rewrite splices
/// `&place` as argument zero so the borrow the requirement declared is the
/// borrow the adapter receives. A mutating `&mut self` requirement
/// (`Owner::name(&mut self, ...)`) forwards the same way: the member call
/// mutably borrows the receiver place for the call, checking already
/// demanded a writable receiver source, and the rewrite splices `&mut
/// place` for the adapter's leading `&mut Owner` parameter. Write-only and
/// qualified receivers stay closed: a `&write self` or `self in Domain`
/// receiver's custody and obligation transfer are separate shapes this row
/// does not settle.
/// Execution consumes the association while Terminal retains the
/// requirement.
fn resolve_top_level_requirement_adapter_row(
    typed: &TypedTrees,
    plan: &effects::provider_plan::ProviderPlan,
    row: &effects::provider_plan::ProviderPlanRow,
    method: &effects::provider_plan::ServiceMethod,
    requirement: &typed_trees::machine::Machine,
    machine_identity: &str,
) -> Result<Vec<ResolvedAdapterRow>, Diagnostic> {
    let requirement_name = requirement.name.as_str();
    if !requirement.symbol.is_valid() || !requirement.attached_data_symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "selected top-level boundary requirement `{requirement_name}` has no exact typed owner symbol",
        )));
    }
    let [entry] = typed.machine_states(requirement) else {
        return Err(Diagnostic::error(format!(
            "selected top-level boundary requirement `{requirement_name}` does not declare exactly one entry signature",
        )));
    };
    if !entry.symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "selected top-level boundary requirement `{requirement_name}` has no exact entry-state symbol",
        )));
    }
    // `self` types as a plain `Named` `Self` reference and `&self`/`&mut
    // self` carry a `Reference` node whose referee is that same `Named`
    // owner; an `in`-qualified receiver carries a `Constrained` node. A
    // shared `&self` borrows the receiver place for the duration of the
    // call, so it forwards `&place` to the adapter's leading `&Owner`
    // parameter -- the same member-call settlement an owned `self` uses,
    // with the borrow recorded on the spliced argument. A mutating `&mut
    // self` takes the same forwarding shape: the member call mutably
    // borrows the place, checking already required a writable receiver
    // source, and the adapter's leading parameter must be `&mut Owner`.
    // Write-only and qualified receivers stay closed here: their custody
    // and obligation transfer are separate shapes this row does not settle.
    let self_receiver = typed
        .state_parameters(entry)
        .iter()
        .find(|parameter| parameter.is_self);
    let receiver_access = match self_receiver {
        Some(self_receiver) => match typed
            .type_reference_table
            .type_reference(self_receiver.type_reference)
        {
            typed_trees::types::TypeReferenceNode::Named { .. } => None,
            typed_trees::types::TypeReferenceNode::Reference {
                referee, access, ..
            } if matches!(
                *access,
                language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable
            ) && named_type_symbol(typed, *referee).is_some() =>
            {
                Some(*access)
            }
            _ => {
                return Err(Diagnostic::error(format!(
                    "selected top-level boundary requirement `{requirement_name}` takes a write-only or qualified `self` receiver; only an owned `self`, shared `&self`, or mutable `&mut self` receiver settles a direct-call dispatch row",
                )));
            }
        },
        None => None,
    };
    if plan.provider_type.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter ProviderPlan `{}` has no nominal provider type",
            plan.name,
        )));
    }
    let adapter = provider_planning::exact_checked_adapter(typed, plan, row)?;
    if typed.attached_data_path(adapter).as_deref() != Some(plan.provider_type.as_str()) {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` does not belong to nominal provider `{}`",
            plan.provider_type,
        )));
    }
    if !adapter.supply_mode.is_checked_body() {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` is not a checked body",
        )));
    }
    if !typed.machine_type_parameters(adapter).is_empty() {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` is generic over {} machine binders; only an exact nongeneric realization can settle a boundary row",
            typed.machine_type_parameters(adapter).len(),
        )));
    }
    let Some(adapter_entry) = typed.machine_states(adapter).first() else {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` has no executable entry state",
        )));
    };
    if !adapter_entry.symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` has no exact entry-state symbol",
        )));
    }
    let conformances = typed
        .machine_trait_conformances(adapter)
        .iter()
        .filter(|conformance| {
            conformance.external_binding.is_none()
                && conformance.symbol == requirement.symbol
                && conformance.requirement_symbol == requirement.symbol
        })
        .count();
    if conformances != 1 {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` binds top-level boundary requirement `{}` through {conformances} checked conformances",
            method.requirement_identity,
        )));
    }
    let actual_parameters: Vec<_> = typed
        .state_parameters(adapter_entry)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect();
    let forward_receiver = match exact_adapter_receiver_shape(
        typed,
        &actual_parameters,
        method.parameter_count,
        requirement.attached_data_symbol,
        receiver_access,
    ) {
        Some(forward_receiver) => forward_receiver,
        None => {
            let count = actual_parameters.len();
            return Err(Diagnostic::error(format!(
                "selected checked adapter `{machine_identity}` has {count} non-self entry parameters; top-level boundary requirement `{}` requires {} or one leading `{}` receiver",
                method.requirement_identity, method.parameter_count, plan.provider_type,
            )));
        }
    };
    if self_receiver.is_some() != forward_receiver {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` receiver shape does not match top-level boundary requirement `{}`: a `self`/`&self`/`&mut self` requirement forwards the receiver place to a leading owner parameter of the same access, a receiver-free requirement takes the exact declared arity",
            method.requirement_identity,
        )));
    }
    // A direct machine call targets the requirement's entry state, exactly
    // as the realization is keyed by its own entry state.
    Ok(vec![ResolvedAdapterRow::Adapter(AdapterRow {
        receiver_trait: requirement.symbol,
        provider_plan_digest: *plan.identity_digest().as_bytes(),
        receiver_trait_name: requirement_name.to_owned(),
        requirement: method.name.clone(),
        requirement_identity: method.requirement_identity.clone(),
        requirement_symbol: entry.symbol,
        adapter_target: adapter.name.as_str().to_owned(),
        symbol: adapter_entry.symbol,
        forward_receiver,
        family_tuple: Box::default(),
        family_tuple_display: Box::default(),
        top_level_owner: Some(requirement.attached_data_symbol),
    })])
}

/// Realize one requirement's authored finite family as one dispatch row per
/// declared roster tuple. One selected conformance must cover the complete
/// family: a tuple without a checked provider specialization would publish a
/// dispatch table silently missing that declared case, so partial coverage
/// leaves the whole requirement dynamically ineligible rather than settling
/// the realized subset.
#[allow(clippy::too_many_arguments)]
fn resolve_family_adapter_row(
    typed: &TypedTrees,
    plan: &effects::provider_plan::ProviderPlan,
    row: &effects::provider_plan::ProviderPlanRow,
    method: &effects::provider_plan::ServiceMethod,
    receiver_trait: &typed_trees::trait_definition::TraitDefinition,
    requirement_owner: &typed_trees::trait_definition::TraitDefinition,
    signature: &typed_trees::signature::StateSignature,
    machine_identity: &str,
) -> Result<Vec<ResolvedAdapterRow>, Diagnostic> {
    let ineligible = |reason: String| {
        vec![ResolvedAdapterRow::GenericRequirement(
            GenericBoundaryRequirement {
                receiver_trait: receiver_trait.symbol,
                provider_plan_digest: *plan.identity_digest().as_bytes(),
                requirement_symbol: signature.symbol,
                requirement_identity: method.requirement_identity.clone(),
                reason,
            },
        )]
    };
    let (arity, tuples) = match finite_signature_family(typed, signature) {
        FamilyProbe::Finite { arity, tuples } => (arity, tuples),
        FamilyProbe::NotFinite(reason) => {
            return Ok(ineligible(format!(
                "finite generic requirement `{}` is ineligible: {reason}",
                method.requirement_identity,
            )));
        }
    };

    if plan.provider_type.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter ProviderPlan `{}` has no nominal provider type",
            plan.name,
        )));
    }
    let adapter = provider_planning::exact_checked_adapter(typed, plan, row)?;
    if typed.attached_data_path(adapter).as_deref() != Some(plan.provider_type.as_str()) {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` does not belong to nominal provider `{}`",
            plan.provider_type,
        )));
    }
    if !adapter.supply_mode.is_checked_body() {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` is not a checked body",
        )));
    }
    // A family provider must be generic over exactly the requirement's
    // enumerable value binders; other binder kinds have no `where` equality
    // to close them and mixed static arguments cannot key a value tuple.
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
    if provider_parameters.len() != provider_value_parameters {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` carries {} non-value machine binders; a finite family can only close the requirement's {arity} value binders",
            provider_parameters.len() - provider_value_parameters,
        )));
    }
    if provider_value_parameters != arity {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` takes {provider_value_parameters} value binders but requirement `{}` declares a family of arity {arity}",
            method.requirement_identity,
        )));
    }
    let Some(template_entry) = typed.machine_states(adapter).first() else {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` has no executable entry state",
        )));
    };

    let conformances = typed
        .machine_trait_conformances(adapter)
        .iter()
        .filter(|conformance| {
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
        .count();
    if conformances != 1 {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` binds exact overload `{}` through {conformances} checked conformances",
            method.requirement_identity,
        )));
    }

    // The template entry's parameter shape is invariant under const
    // specialization, so the receiver check is computed once on the template.
    let actual_parameters = typed
        .state_parameters(template_entry)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    let forward_receiver = match exact_adapter_receiver_shape(
        typed,
        &actual_parameters,
        method.parameter_count,
        requirement_owner.symbol,
        None,
    ) {
        Some(forward_receiver) => forward_receiver,
        None => {
            let count = actual_parameters.len();
            return Err(Diagnostic::error(format!(
                "selected checked adapter `{machine_identity}` has {count} non-self entry parameters; exact overload `{}` requires {} or one leading `{}` receiver",
                method.requirement_identity, method.parameter_count, requirement_owner.name,
            )));
        }
    };

    let mut rows = Vec::new();
    let mut missing = Vec::new();
    for tuple in &tuples {
        // A roster row is filled only by a bare value-tuple specialization of
        // the selected provider's own template: retained const identities equal
        // the tuple exactly (wrong widths never fill), and every non-value
        // argument coordinate stays empty (a shape-substituted record —
        // type, machine, or conformance arguments beside the value tuple —
        // never fills). A specialization of a sibling provider's template is
        // ineligible even at the same tuple: one selected conformance covers
        // the roster alone.
        let mut specializations = typed
            .machine_specializations
            .iter()
            .filter(|specialization| {
                specialization.template == adapter.symbol
                    && specialization.const_argument_identities.as_slice()
                        == tuple.identities.as_ref()
                    && specialization.type_argument_identities.is_empty()
                    && specialization.machine_arguments.is_empty()
                    && specialization.conformance_arguments.is_empty()
                    && specialization.inferred_conformance_arguments.is_empty()
                    && specialization.conformance_applications.is_empty()
            });
        let Some(specialization) = specializations.next() else {
            missing.push(format!("({})", tuple.display.join(", ")));
            continue;
        };
        if specializations.next().is_some() {
            return Err(Diagnostic::error(format!(
                "finite family tuple `({})` of requirement `{}` resolves to multiple retained specializations of `{machine_identity}`",
                tuple.display.join(", "),
                method.requirement_identity,
            )));
        }
        let Some(instance) = typed
            .machines()
            .iter()
            .find(|machine| machine.symbol == specialization.instance)
        else {
            return Err(Diagnostic::error(format!(
                "finite family specialization of `{machine_identity}` for tuple `({})` has no retained machine instance",
                tuple.display.join(", "),
            )));
        };
        let Some(entry) = typed.machine_states(instance).first() else {
            return Err(Diagnostic::error(format!(
                "finite family specialization of `{machine_identity}` for tuple `({})` has no executable entry state",
                tuple.display.join(", "),
            )));
        };
        if !entry.symbol.is_valid() {
            return Err(Diagnostic::error(format!(
                "finite family specialization of `{machine_identity}` for tuple `({})` has no exact entry-state symbol",
                tuple.display.join(", "),
            )));
        }
        rows.push(ResolvedAdapterRow::Adapter(AdapterRow {
            receiver_trait: receiver_trait.symbol,
            provider_plan_digest: *plan.identity_digest().as_bytes(),
            receiver_trait_name: receiver_trait.name.as_str().to_owned(),
            requirement: method.name.clone(),
            requirement_identity: method.requirement_identity.clone(),
            requirement_symbol: signature.symbol,
            adapter_target: instance.name.as_str().to_owned(),
            symbol: entry.symbol,
            forward_receiver,
            family_tuple: tuple.identities.clone(),
            family_tuple_display: tuple.display.clone(),
            top_level_owner: None,
        }));
    }
    // A dynamic family publishes every declared roster tuple or none: the
    // requirement, not the provider's realized subset, owns the roster, and
    // an absent row would otherwise disappear silently from the table.
    if !missing.is_empty() {
        let coverage = if rows.is_empty() {
            "no checked provider specialization exists for any of them".to_owned()
        } else {
            format!("the selected provider realizes only {} of them", rows.len())
        };
        return Ok(ineligible(format!(
            "finite generic requirement `{}` declares {} tuples but {coverage}; partial provider coverage cannot supply the dynamic family (missing: {})",
            method.requirement_identity,
            tuples.len(),
            missing.join(", "),
        )));
    }
    Ok(rows)
}
