//! Adapter rows, generic boundary requirements and their resolution.

use crate::boundary_dispatch::boundary_fields::exact_boundary_trait;
use crate::boundary_dispatch::signature_families::{FamilyProbe, finite_signature_family};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
mod closed_trait_applications;

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
    /// Finite-family tuple this row realizes: canonical const identities in
    /// the requirement's value-binder declaration order (the same strings a
    /// `MachineSpecialization` retains in `const_argument_identities`). Empty
    /// on an exact nongeneric row, which matches every call to its
    /// requirement without consulting static arguments.
    pub(crate) family_tuple: Box<[String]>,
    /// Readable tuple spellings for diagnostics only; `family_tuple` is the
    /// semantic key.
    pub(crate) family_tuple_display: Box<[String]>,
    pub(crate) receiver: AdapterReceiver,
}

/// Service requirements dispatch through a selected receiver. Direct machine
/// requirements key their own entry; a nominal namespace is optional source
/// qualification, never a prerequisite for supplying that exact requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdapterReceiver {
    Service,
    Direct { namespace: symbols::SymbolHandle },
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
    let provider_context =
        provider_planning::ProviderPlanningContext::new(typed, source::DependencyScope::Product);
    if typed.operators().iter().any(|operator| {
        provider_planning::service_schema::schema_binds_exact_boundary_operator(
            provider_context,
            &plan.schema,
            operator,
        )
    }) {
        return Ok(None);
    }
    if !row.requirement_identity.is_valid() {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter row `{}` in ProviderPlan `{}` has no exact overload identity",
            row.method, plan.name,
        )));
    }
    let methods = plan
        .schema
        .methods
        .iter()
        .filter(|method| plan.schema.row_binds_method(row, method, &plan.identities))
        .collect::<Vec<_>>();
    let [method] = methods.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter row `{}` / `{}` in ProviderPlan `{}` binds {} exact schema methods",
            row.method,
            plan.identities.text(row.requirement_identity),
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
                provider_context,
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
                plan.schema.identity_text(plan.schema.trait_name),
                many.len(),
            )));
        }
    }

    let receiver_trait = exact_boundary_trait(
        typed,
        plan.schema.identity_text(plan.schema.trait_name),
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
                && typed.trait_requirement_callable_identity_text(requirement_owner, signature)
                    == plan.schema.identity_text(method.requirement_identity)
        })
        .collect::<Vec<_>>();
    let [signature] = signatures.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter requirement `{}` resolves to {} exact typed signatures",
            plan.schema.identity_text(method.requirement_identity),
            signatures.len(),
        )));
    };
    if !signature.symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter requirement `{}` has no exact typed symbol",
            plan.schema.identity_text(method.requirement_identity),
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
    declarations: &typed_trees::references::DeclarationSelection,
    plan: &effects::provider_plan::ProviderPlan,
    row: &effects::provider_plan::ProviderPlanRow,
    specialization_rows: &[effects::SelectedRequirementSpecialization],
    specialization_identities: &terminal_psi::TerminalIdentityTable,
) -> Result<Vec<ResolvedAdapterRow>, Diagnostic> {
    let rows = resolve_template_adapter_rows(
        typed,
        declarations,
        plan,
        row,
        specialization_rows,
        specialization_identities,
    )?;
    closed_trait_applications::project(
        typed,
        declarations,
        plan,
        row,
        specialization_rows,
        specialization_identities,
        rows,
    )
}

fn resolve_template_adapter_rows(
    typed: &TypedTrees,
    declarations: &typed_trees::references::DeclarationSelection,
    plan: &effects::provider_plan::ProviderPlan,
    row: &effects::provider_plan::ProviderPlanRow,
    specialization_rows: &[effects::SelectedRequirementSpecialization],
    specialization_identities: &terminal_psi::TerminalIdentityTable,
) -> Result<Vec<ResolvedAdapterRow>, Diagnostic> {
    use effects::provider_plan::ProviderBinding;

    let ProviderBinding::CheckedAdapter {
        machine_identity, ..
    } = &row.binding
    else {
        return Ok(Vec::new());
    };
    let machine_identity = plan.identity_text(*machine_identity);
    let (method, receiver_trait, requirement_owner, signature) =
        match resolve_selected_adapter_target(typed, plan, row)? {
            Some(SelectedAdapterTarget::TopLevelRequirement {
                method,
                requirement,
            }) => {
                return resolve_top_level_requirement_adapter_row(
                    typed,
                    declarations,
                    plan,
                    row,
                    method,
                    requirement,
                    machine_identity,
                    specialization_rows,
                    specialization_identities,
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

    let (adapter, entry) = planned_checked_adapter(typed, plan, row, machine_identity)?;
    require_nongeneric_adapter(typed, adapter, machine_identity)?;

    if !provider_planning::provider_trait_requirement_matches(
        provider_planning::ProviderPlanningContext::new(typed, source::DependencyScope::Product),
        plan,
        row,
        adapter,
        requirement_owner.symbol,
        signature.symbol,
    ) {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` has no exact selected provider association for overload `{}`",
            plan.schema.identity_text(method.requirement_identity),
        )));
    }

    let actual_parameters = typed.state_parameters(entry);
    require_parameter_count(plan, method, actual_parameters.len())?;

    Ok(vec![ResolvedAdapterRow::Adapter(AdapterRow {
        receiver_trait: receiver_trait.symbol,
        provider_plan_digest: *plan.identity_digest().as_bytes(),
        receiver_trait_name: receiver_trait.name.as_str().to_owned(),
        requirement: method.name.clone(),
        requirement_identity: plan
            .schema
            .identity_text(method.requirement_identity)
            .to_owned(),
        requirement_symbol: signature.symbol,
        adapter_target: adapter.name.as_str().to_owned(),
        symbol: entry.symbol,
        family_tuple: Box::default(),
        family_tuple_display: Box::default(),
        receiver: AdapterReceiver::Service,
    })])
}

fn require_parameter_count(
    plan: &effects::provider_plan::ProviderPlan,
    method: &effects::provider_plan::ServiceMethod,
    actual: usize,
) -> Result<(), Diagnostic> {
    if actual != method.parameter_count {
        return Err(Diagnostic::error(format!(
            "selected checked adapter for `{}` has {} entry parameters; expected exactly {}",
            plan.schema.identity_text(method.requirement_identity),
            actual,
            method.parameter_count,
        )));
    }
    Ok(())
}

/// Resolve the exact checked adapter realizing a top-level `boundary
/// requirement`. A public requirement carrying no static generic binders
/// (`public boundary requirement Owner::name(...);`, optionally with an erased
/// lifetime telescope `Owner::name<'a>(...)`) settles through a checked-body
/// adapter with no static generic binders whose exact `satisfies` edge rejoins
/// that requirement symbol with the same explicit arity. Unlike the trait
/// rows above, top-level machine requirements are not slots in named trait
/// implementation maps. A generic token-machine requirement instead settles
/// through its selected specialization roster: the plan row stays keyed by
/// the authored template and every demanded closed requirement instance emits
/// one row naming the exact specialized provider instance the roster joins to
/// it. The requirement's lifetime telescope is the adapter's own telescope by
/// the identity application conformance checked, so an erased telescope needs
/// no row argument of its own. A receiver-free requirement admits only an
/// exact-arity adapter. Its selected entry is the dispatch identity; an
/// authored nominal qualifier may also identify that direct call, but a
/// structural collection requirement has no nominal owner. Execution consumes
/// the association while Terminal retains the requirement.
fn resolve_top_level_requirement_adapter_row(
    typed: &TypedTrees,
    declarations: &typed_trees::references::DeclarationSelection,
    plan: &effects::provider_plan::ProviderPlan,
    row: &effects::provider_plan::ProviderPlanRow,
    method: &effects::provider_plan::ServiceMethod,
    requirement: &typed_trees::machine::Machine,
    machine_identity: &str,
    specialization_rows: &[effects::SelectedRequirementSpecialization],
    specialization_identities: &terminal_psi::TerminalIdentityTable,
) -> Result<Vec<ResolvedAdapterRow>, Diagnostic> {
    let requirement_name = requirement.name.as_str();
    if !requirement.symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "selected top-level boundary requirement `{requirement_name}` has no exact typed requirement symbol",
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
    let (adapter, adapter_entry) = planned_checked_adapter(typed, plan, row, machine_identity)?;
    if !typed.machine_type_parameters(requirement).is_empty() {
        return resolve_specialized_top_level_requirement_adapter_rows(
            typed,
            declarations,
            plan,
            row,
            method,
            requirement,
            adapter,
            machine_identity,
            specialization_rows,
            specialization_identities,
        );
    }
    require_nongeneric_adapter(typed, adapter, machine_identity)?;
    sole_boundary_conformance(
        typed,
        adapter,
        requirement,
        machine_identity,
        plan,
        method,
        "requirement",
    )?;
    let actual_parameters = typed.state_parameters(adapter_entry);
    require_parameter_count(plan, method, actual_parameters.len())?;
    // A direct machine call targets the requirement's entry state, exactly
    // as the realization is keyed by its own entry state.
    Ok(vec![ResolvedAdapterRow::Adapter(AdapterRow {
        receiver_trait: requirement.symbol,
        provider_plan_digest: *plan.identity_digest().as_bytes(),
        receiver_trait_name: requirement_name.to_owned(),
        requirement: method.name.clone(),
        requirement_identity: plan
            .schema
            .identity_text(method.requirement_identity)
            .to_owned(),
        requirement_symbol: entry.symbol,
        adapter_target: adapter.name.as_str().to_owned(),
        symbol: adapter_entry.symbol,
        family_tuple: Box::default(),
        family_tuple_display: Box::default(),
        receiver: AdapterReceiver::Direct {
            namespace: requirement.attached_data_symbol,
        },
    })])
}

/// Realize a generic top-level `boundary requirement` as one exact dispatch
/// row per demanded closed requirement instance. The selected plan row stays
/// keyed by the authored template; the selected-requirement specialization
/// roster is the only authority joining that row to the closed requirement
/// and provider identities checking minted, so settlement replays roster
/// rows and re-verifies each cited machine against the retained
/// specialization receipts rather than reconstructing the join by name or
/// signature. Every closed instance is nongeneric, so the emitted rows carry
/// no family tuple: they key the closed requirement's own entry state, which
/// is exactly the target a specialized direct call retains. The template's
/// receiver shape is invariant under specialization and was computed once by
/// the caller; each row re-verifies it against the closed requirement's own
/// entry.
#[allow(clippy::too_many_arguments)]
fn resolve_specialized_top_level_requirement_adapter_rows(
    typed: &TypedTrees,
    declarations: &typed_trees::references::DeclarationSelection,
    plan: &effects::provider_plan::ProviderPlan,
    row: &effects::provider_plan::ProviderPlanRow,
    method: &effects::provider_plan::ServiceMethod,
    requirement: &typed_trees::machine::Machine,
    adapter: &typed_trees::machine::Machine,
    machine_identity: &str,
    specialization_rows: &[effects::SelectedRequirementSpecialization],
    specialization_identities: &terminal_psi::TerminalIdentityTable,
) -> Result<Vec<ResolvedAdapterRow>, Diagnostic> {
    // This is a top-level machine requirement, not a named trait-map row.
    // Its template owns the authored satisfaction edge; specialization
    // instantiates that conformance on each closed instance. The roster
    // carries the instance join, while this check authenticates its origin.
    sole_boundary_conformance(
        typed,
        adapter,
        requirement,
        machine_identity,
        plan,
        method,
        "requirement template",
    )?;
    let mut resolved = Vec::new();
    for specialization in specialization_rows.iter().filter(|specialization| {
        specialization.provider_plan_report_identity == plan.report_fingerprint()
            && specialization_identities.text(specialization.selected_requirement_identity)
                == Some(plan.identities.text(row.requirement_identity))
            && specialization_identities.text(specialization.selected_machine_identity)
                == Some(machine_identity)
    }) {
        // Each roster row's retained identities are read once: the closed
        // requirement's feeds the exact-machine lookup, and the provider
        // instance's names that machine in the diagnostics below.
        let requirement_identity = specialization_identities
            .text(specialization.requirement_identity)
            .unwrap_or_default();
        let instance_identity = specialization_identities
            .text(specialization.machine_identity)
            .unwrap_or_default();
        let Some(closed) = typed.machine_by_callable_identity(
            typed.callable_identity_by_text(requirement_identity),
            declarations,
        ) else {
            return Err(Diagnostic::error(format!(
                "selected provider plan `{}` specialization roster names closed requirement `{}` with no exact typed machine",
                plan.name, requirement_identity,
            )));
        };
        let closed_receipts = typed
            .machine_specializations
            .iter()
            .filter(|receipt| {
                receipt.template == requirement.symbol && receipt.instance == closed.symbol
            })
            .count();
        if closed_receipts != 1
            || closed.supply_mode != language_semantics::MachineSupplyMode::TopLevelRequirement
            || !typed.machine_type_parameters(closed).is_empty()
        {
            return Err(Diagnostic::error(format!(
                "selected specialization row `{}` does not name one retained closed instance of requirement template `{machine_identity}`",
                requirement_identity,
            )));
        }
        let [closed_entry] = typed.machine_states(closed) else {
            return Err(Diagnostic::error(format!(
                "closed requirement instance `{}` does not declare exactly one entry signature",
                closed.name,
            )));
        };
        if !closed_entry.symbol.is_valid() {
            return Err(Diagnostic::error(format!(
                "closed requirement instance `{}` has no exact entry-state symbol",
                closed.name,
            )));
        }
        let Some(instance) = typed.machine_by_callable_identity(
            typed.callable_identity_by_text(instance_identity),
            declarations,
        ) else {
            return Err(Diagnostic::error(format!(
                "selected provider plan `{}` specialization roster names provider instance `{}` with no exact typed machine",
                plan.name, instance_identity,
            )));
        };
        let instance_receipts = typed
            .machine_specializations
            .iter()
            .filter(|receipt| {
                receipt.template == adapter.symbol && receipt.instance == instance.symbol
            })
            .count();
        if instance_receipts != 1 || !typed.machine_type_parameters(instance).is_empty() {
            return Err(Diagnostic::error(format!(
                "selected specialization row `{instance_identity}` does not name one retained specialization instance of checked adapter `{machine_identity}`",
            )));
        }
        let instance_entry = crate::exact_checked_body_entry(
            typed,
            plan,
            row,
            instance,
            &format!("selected provider instance `{instance_identity}`"),
        )?;
        let actual_parameters = typed.state_parameters(instance_entry);
        require_parameter_count(plan, method, actual_parameters.len())?;
        resolved.push(ResolvedAdapterRow::Adapter(AdapterRow {
            receiver_trait: closed.symbol,
            provider_plan_digest: *plan.identity_digest().as_bytes(),
            receiver_trait_name: closed.name.as_str().to_owned(),
            requirement: method.name.clone(),
            requirement_identity: requirement_identity.to_owned(),
            requirement_symbol: closed_entry.symbol,
            adapter_target: instance.name.as_str().to_owned(),
            symbol: instance_entry.symbol,
            family_tuple: Box::default(),
            family_tuple_display: Box::default(),
            receiver: AdapterReceiver::Direct {
                namespace: closed.attached_data_symbol,
            },
        }));
    }
    // Coverage is per selected row, not per roster: every retained closed
    // requirement instance of this template exists because checked demand
    // applied it, so a demanded instance the roster does not rebind to this
    // plan's provider would silently reach the unbound template.
    let retained = typed
        .machine_specializations
        .iter()
        .filter(|receipt| receipt.template == requirement.symbol)
        .filter_map(|receipt| typed.machine(receipt.instance))
        .filter(|instance| {
            instance.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
        })
        .count();
    if resolved.len() != retained {
        return Err(Diagnostic::error(format!(
            "selected provider plan `{}` settles {} specialization rows for requirement template `{}` but checking retained {retained} closed requirement instances",
            plan.name,
            resolved.len(),
            plan.schema.identity_text(method.requirement_identity),
        )));
    }
    Ok(resolved)
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
                requirement_identity: plan
                    .schema
                    .identity_text(method.requirement_identity)
                    .to_owned(),
                reason,
            },
        )]
    };
    let (arity, tuples) = match finite_signature_family(typed, signature) {
        FamilyProbe::Finite { arity, tuples } => (arity, tuples),
        FamilyProbe::NotFinite(reason) => {
            return Ok(ineligible(format!(
                "finite generic requirement `{}` is ineligible: {reason}",
                plan.schema.identity_text(method.requirement_identity),
            )));
        }
    };

    let (adapter, template_entry) = planned_checked_adapter(typed, plan, row, machine_identity)?;
    // A family provider must be generic over exactly the requirement's
    // enumerable value binders; other binder kinds have no `where` equality
    // to close them and mixed static arguments cannot key a value tuple.
    let provider_parameters = typed.machine_type_parameters(adapter);
    let provider_value_parameters = provider_parameters
        .iter()
        .filter(|parameter| parameter.kind.is_const_or_value())
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
            plan.schema.identity_text(method.requirement_identity),
        )));
    }
    if !provider_planning::provider_trait_requirement_matches(
        provider_planning::ProviderPlanningContext::new(typed, source::DependencyScope::Product),
        plan,
        row,
        adapter,
        requirement_owner.symbol,
        signature.symbol,
    ) {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` has no exact selected provider association for overload `{}`",
            plan.schema.identity_text(method.requirement_identity),
        )));
    }

    // The template entry's parameter shape is invariant under const
    // specialization, so the arity check is computed once on the template.
    let actual_parameters = typed.state_parameters(template_entry);
    require_parameter_count(plan, method, actual_parameters.len())?;

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
                    && specialization.is_const_only()
            });
        let Some(specialization) = specializations.next() else {
            missing.push(format!("({})", tuple.display.join(", ")));
            continue;
        };
        if specializations.next().is_some() {
            return Err(Diagnostic::error(format!(
                "finite family tuple `({})` of requirement `{}` resolves to multiple retained specializations of `{machine_identity}`",
                tuple.display.join(", "),
                plan.schema.identity_text(method.requirement_identity),
            )));
        }
        let Some(instance) = typed.machine(specialization.instance) else {
            return Err(Diagnostic::error(format!(
                "finite family specialization of `{machine_identity}` for tuple `({})` has no retained machine instance",
                tuple.display.join(", "),
            )));
        };
        let entry = crate::exact_checked_body_entry(
            typed,
            plan,
            row,
            instance,
            &format!(
                "finite family specialization of `{machine_identity}` for tuple `({})`",
                tuple.display.join(", "),
            ),
        )?;
        rows.push(ResolvedAdapterRow::Adapter(AdapterRow {
            receiver_trait: receiver_trait.symbol,
            provider_plan_digest: *plan.identity_digest().as_bytes(),
            receiver_trait_name: receiver_trait.name.as_str().to_owned(),
            requirement: method.name.clone(),
            requirement_identity: plan
                .schema
                .identity_text(method.requirement_identity)
                .to_owned(),
            requirement_symbol: signature.symbol,
            adapter_target: instance.name.as_str().to_owned(),
            symbol: entry.symbol,
            family_tuple: tuple.identities.clone(),
            family_tuple_display: tuple.display.clone(),
            receiver: AdapterReceiver::Service,
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
            plan.schema.identity_text(method.requirement_identity),
            tuples.len(),
            missing.join(", "),
        )));
    }
    Ok(rows)
}

/// The exact checked adapter a boundary row's ProviderPlan names, with the
/// entry state that starts it: the plan must carry a nominal provider type,
/// and the resolved adapter must be that provider's exact executable checked
/// body.
fn planned_checked_adapter<'typed>(
    typed: &'typed TypedTrees,
    plan: &effects::provider_plan::ProviderPlan,
    row: &effects::provider_plan::ProviderPlanRow,
    machine_identity: &str,
) -> Result<
    (
        &'typed typed_trees::machine::Machine,
        &'typed typed_trees::state::State,
    ),
    Diagnostic,
> {
    if plan.provider_type.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter ProviderPlan `{}` has no nominal provider type",
            plan.name,
        )));
    }
    let adapter = provider_planning::exact_checked_adapter(
        provider_planning::ProviderPlanningContext::new(typed, source::DependencyScope::Product),
        plan,
        row,
    )?;
    let entry = crate::exact_checked_body_entry(
        typed,
        plan,
        row,
        adapter,
        &format!("selected checked adapter `{machine_identity}`"),
    )?;
    Ok((adapter, entry))
}

/// A generic adapter cannot be the exact realization of a nongeneric
/// requirement: its unbound binders have no tuple to close them.
fn require_nongeneric_adapter(
    typed: &TypedTrees,
    adapter: &typed_trees::machine::Machine,
    machine_identity: &str,
) -> Result<(), Diagnostic> {
    if !typed.machine_type_parameters(adapter).is_empty() {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` is generic over {} machine binders; only an exact nongeneric realization can settle a boundary row",
            typed.machine_type_parameters(adapter).len(),
        )));
    }
    Ok(())
}

/// The selected checked adapter must bind the top-level boundary
/// requirement through exactly one internal conformance.
fn sole_boundary_conformance(
    typed: &TypedTrees,
    adapter: &typed_trees::machine::Machine,
    requirement: &typed_trees::machine::Machine,
    machine_identity: &str,
    plan: &effects::provider_plan::ProviderPlan,
    method: &effects::provider_plan::ServiceMethod,
    spelled: &str,
) -> Result<(), Diagnostic> {
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
            "selected checked adapter `{machine_identity}` binds top-level boundary {spelled} `{}` through {conformances} checked conformances",
            plan.schema.identity_text(method.requirement_identity),
        )));
    }
    Ok(())
}
