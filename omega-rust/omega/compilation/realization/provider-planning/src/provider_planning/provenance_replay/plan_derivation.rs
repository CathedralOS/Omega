//! Deriving satisfies plans, top-level requirement plans and boundary
//! operator plans with their provenance.
//!
//! Provider paths use the same complete nominal projection as Build selection
//! and Terminal candidate emission. They do not replace declaration custody:
//! grouping/provenance retain exact provider symbols, packages and normalized
//! callable identities, which independent replay must still rejoin.
//!
//! Plans belong to authored satisfaction declarations, not the executable
//! instances specialization mints from them. All requirement schema kinds use
//! that ownership rule; the closed-requirement roster retains and joins every
//! executable tuple separately, without inflating original schema coverage.

use crate::ProviderPlanningContext;
use crate::provider_identity::{
    provider_owner_package_identity, provider_owner_path, provider_owner_symbol,
};
use crate::provider_planning::provenance_replay::requirement_identities::{
    exact_satisfied_requirement_identity, inferred_compiler_leaf_binding,
};
use crate::provider_planning::provenance_replay::{
    DerivedProviderPlan, ProviderPlanProvenance, ProviderSchemaDeclaration,
    SelectedTargetMachineOrigin, satisfies_plan_name,
};
use crate::provider_planning::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema};
use effects::provider_plan::{ProviderIdentityId, ProviderIdentityTable};

pub(crate) fn derive_provider_plans(
    context: ProviderPlanningContext<'_>,
    calling_plans: &[crate::calling_policy_plans::BoundaryCallingPlanRealization],
    selected_target: Option<&str>,
    target_machine_origins: &[SelectedTargetMachineOrigin],
) -> Result<Vec<DerivedProviderPlan>, Vec<diagnostics::Diagnostic>> {
    let typed = context.typed;
    let mut plans: Vec<DerivedProviderPlan> = Vec::new();
    // Typed trees retain every authored body and its conformances. This
    // transitional settlement derives only the requested realization's rows;
    // semantic declaration queries must never perform that selection for us.
    for machine in typed.machines() {
        if !is_authored_provider_machine(typed, machine)
            || !crate::target_machines::realizes_machine(context, machine, selected_target)
        {
            continue;
        }
        let origin_package_identity = typed.symbols.symbol_package_identity(machine.symbol);
        let provider_type_package_identity = provider_owner_package_identity(typed, machine);
        let provider_type_symbol = provider_owner_symbol(typed, machine);
        for clause in typed.machine_trait_conformances(machine) {
            if clause.requirement.is_some()
                && checked_trees::typed_trees::operator::resolve_satisfied_boundary_operator_for_conformance(
                    typed, machine, clause,
                )
                .is_some()
            {
                // Exact boundary-operator requirements use one overloaded
                // signature per provider slot; derive them below rather than
                // manufacturing an empty boundary-trait schema here.
                continue;
            }
            // Ordinary checked adapters and sealed compiler leaves retain
            // requirement-named satisfaction edges. Build-owned native supply
            // joins through admitted native rows, without forwarding machines.
            // Whole-trait conformances derive their complete named map below.
            let Some(requirement) = clause.requirement.as_ref() else {
                continue;
            };
            if machine.supply_mode.is_checked_body()
                && !typed
                    .traits()
                    .iter()
                    .any(|definition| definition.is_boundary && definition.symbol == clause.symbol)
            {
                continue;
            }
            let Some((row_binding, target_machine_origin)) = checked_or_catalog_binding(
                typed,
                machine,
                clause,
                selected_target,
                target_machine_origins,
            ) else {
                continue;
            };
            let target = selected_target.unwrap_or_default().to_owned();
            let provider_type = provider_owner_path(typed, machine).unwrap_or_default();
            let semantic_requirement_identity = exact_satisfied_requirement_identity(
                typed,
                clause.symbol,
                clause.requirement_symbol,
            );
            let requirement_symbol = clause.requirement_symbol;
            if let Some((schema_declaration, schema_trait, schema)) =
                provider_plan_schema_target(context, calling_plans, clause.symbol)
            {
                let plan_name = satisfies_plan_name(&target, &schema_trait, &provider_type);
                let position = derived_plan_position(
                    &mut plans,
                    plan_name,
                    provider_type.clone(),
                    provider_type_package_identity,
                    target.clone(),
                    schema,
                    schema_declaration,
                    provider_type_symbol,
                    origin_package_identity,
                );
                debug_assert_eq!(plans[position].provenance.schema, schema_declaration);
                debug_assert_eq!(
                    plans[position].provenance.provider_type,
                    provider_type_symbol
                );
                let binding = row_binding.resolve(&mut plans[position].plan.identities);
                let requirement_identity = mint_plan_row_identity(
                    &mut plans[position].plan,
                    &semantic_requirement_identity,
                );
                plans[position].plan.rows.push(ProviderPlanRow {
                    method: requirement.as_str().to_owned(),
                    requirement_identity,
                    requirement_lifetime_partition:
                        language_semantics::type_identity::normalize_requirement_lifetime_partition(
                            &clause.trait_lifetime_arguments,
                        ),
                    admitted_same_stack_bytes: 0,
                    binding,
                });
                plans[position]
                    .provenance
                    .row_requirements
                    .push(requirement_symbol);
                plans[position]
                    .provenance
                    .row_realizations
                    .push(super::ProviderRowRealization::Authored(machine.symbol));
                plans[position]
                    .provenance
                    .row_target_machine_origins
                    .push(target_machine_origin.clone());
                plans[position].provenance.row_placed_accessors.push(None);
            }
        }
    }
    plans.extend(derive_boundary_operator_plans(
        context,
        selected_target,
        target_machine_origins,
    ));
    plans.extend(derive_top_level_requirement_plans(
        context,
        selected_target,
        target_machine_origins,
    ));
    let named =
        super::named_provider_plans::derive(context, calling_plans, selected_target, &plans);
    plans.extend(named);
    derive_placed_accessor_plans(context, selected_target, &mut plans)?;
    // Coverage includes the selected target's direct body closure, not only
    // the portable adapter's own declaration. Keep every authored source body,
    // but exclude a whole map when one of its rows reaches a known absent
    // target body. Completion, provider dependencies and admission remain
    // separate obligations on the candidates that selection actually chooses.
    let absent = crate::target_machines::DirectTargetBodyAbsences::derive(context, selected_target);
    plans.retain(|candidate| {
        !candidate.provenance.row_realizations.iter().any(|machine| {
            machine
                .authored()
                .is_some_and(|machine| absent.contains(typed, machine))
        })
    });
    Ok(plans)
}

fn derive_top_level_requirement_plans(
    context: ProviderPlanningContext<'_>,
    selected_target: Option<&str>,
    target_machine_origins: &[SelectedTargetMachineOrigin],
) -> Vec<DerivedProviderPlan> {
    let typed = context.typed;
    let mut plans = Vec::<DerivedProviderPlan>::new();
    for machine in typed.machines() {
        if !is_authored_provider_machine(typed, machine)
            || !crate::target_machines::realizes_machine(context, machine, selected_target)
        {
            continue;
        }
        let is_checked_adapter = machine.supply_mode
            == language_semantics::MachineSupplyMode::CheckedBody
            && machine.body_is_present;
        let is_external_leaf = machine.supply_mode
            == language_semantics::MachineSupplyMode::Boundary
            && !machine.body_is_present;
        if !is_checked_adapter && !is_external_leaf {
            continue;
        }
        let Some(provider_type_symbol) = provider_owner_symbol(typed, machine) else {
            continue;
        };
        let Some(provider_type) = provider_owner_path(typed, machine) else {
            continue;
        };
        let provider_type_package_identity = provider_owner_package_identity(typed, machine);
        let origin_package_identity = typed.symbols.symbol_package_identity(machine.symbol);
        for clause in typed.machine_trait_conformances(machine) {
            // The authored satisfies edge names the requirement declaration
            // the provider claims: for a generic token-machine template the
            // row retains that authored template identity because provider
            // plans are selected before specialization settles the closed
            // applications. The selected-requirement specialization roster
            // is the exact join to each closed instance Terminal Psi mints;
            // derivation neither aliases the closed requirement to its
            // template nor trusts an equal runtime signature.
            let Some(requirement) = typed.machines().iter().find(|candidate| {
                candidate.symbol == clause.symbol
                    && clause.requirement_symbol == candidate.symbol
                    && candidate.supply_mode
                        == language_semantics::MachineSupplyMode::TopLevelRequirement
            }) else {
                continue;
            };
            let Some(schema) =
                crate::service_schema::from_typed_boundary_requirement(context, requirement)
            else {
                continue;
            };
            let requirement_identity = typed
                .machine_callable_identity_text(requirement)
                .unwrap_or_default();
            let [schema_method] = schema.methods.as_slice() else {
                continue;
            };
            if requirement_identity.is_empty()
                || schema.identity_text(schema_method.requirement_identity) != requirement_identity
            {
                continue;
            }
            let Some((binding, target_machine_origin)) = checked_or_catalog_binding(
                typed,
                machine,
                clause,
                selected_target,
                target_machine_origins,
            ) else {
                continue;
            };
            let target = selected_target.unwrap_or_default().to_owned();
            let plan_name = satisfies_plan_name(
                &target,
                schema.identity_text(schema.trait_name),
                &provider_type,
            );
            let position = derived_plan_position(
                &mut plans,
                plan_name,
                provider_type.clone(),
                provider_type_package_identity,
                target.clone(),
                schema.clone(),
                ProviderSchemaDeclaration::BoundaryRequirement(requirement.symbol),
                Some(provider_type_symbol),
                origin_package_identity,
            );
            let binding = binding.resolve(&mut plans[position].plan.identities);
            let requirement_identity =
                mint_plan_row_identity(&mut plans[position].plan, &requirement_identity);
            plans[position].plan.rows.push(ProviderPlanRow {
                method: schema_method.name.clone(),
                requirement_identity,
                requirement_lifetime_partition: Vec::new(),
                admitted_same_stack_bytes: 0,
                binding,
            });
            plans[position]
                .provenance
                .row_requirements
                .push(requirement.symbol);
            plans[position]
                .provenance
                .row_realizations
                .push(super::ProviderRowRealization::Authored(machine.symbol));
            plans[position]
                .provenance
                .row_target_machine_origins
                .push(target_machine_origin.clone());
            plans[position].provenance.row_placed_accessors.push(None);
        }
    }
    plans
}

/// The boundary schema under which an exact requirement edge is installed:
/// the edge's own boundary trait, with no whole-trait application. A root that
/// inherits the requirement and adds `Calling<C>` is installed through a named
/// implementation of that root, whose plan `derive_named_provider_plans`
/// derives from its explicit rows; an exact edge never claims a descendant
/// schema on behalf of the data that owns its machine.
fn provider_plan_schema_target(
    context: ProviderPlanningContext<'_>,
    calling_plans: &[crate::calling_policy_plans::BoundaryCallingPlanRealization],
    satisfied_trait_symbol: symbols::SymbolHandle,
) -> Option<(ProviderSchemaDeclaration, String, ServiceSchema)> {
    let definition =
        context.typed.traits().iter().find(|definition| {
            definition.is_boundary && definition.symbol == satisfied_trait_symbol
        })?;
    let schema =
        crate::service_schema::from_typed_instance(context, calling_plans, definition, &[])?;
    Some((
        ProviderSchemaDeclaration::BoundaryTrait(definition.symbol),
        schema.identity_text(schema.trait_name).to_owned(),
        schema,
    ))
}

fn derive_boundary_operator_plans(
    context: ProviderPlanningContext<'_>,
    selected_target: Option<&str>,
    target_machine_origins: &[SelectedTargetMachineOrigin],
) -> Vec<DerivedProviderPlan> {
    let typed = context.typed;
    let mut plans = Vec::<DerivedProviderPlan>::new();
    for machine in typed.machines() {
        if !is_authored_provider_machine(typed, machine)
            || !crate::target_machines::realizes_machine(context, machine, selected_target)
        {
            continue;
        }
        let origin_package_identity = typed.symbols.symbol_package_identity(machine.symbol);
        let provider_type_package_identity = provider_owner_package_identity(typed, machine);
        let provider_type_symbol = provider_owner_symbol(typed, machine);
        for clause in typed.machine_trait_conformances(machine) {
            if clause.requirement.is_none() {
                continue;
            }
            let Some(operator) =
                checked_trees::typed_trees::operator::resolve_satisfied_boundary_operator_for_conformance(
                    typed, machine, clause,
                )
            else {
                continue;
            };
            // A token-bearing requirement has a signature view for overload
            // matching, not a second provider slot. Its machine declaration
            // owns the schema and provenance in derive_top_level_requirement_plans.
            if typed.machine(operator.symbol).is_some() {
                continue;
            }
            let Some((binding, target_machine_origin)) = checked_or_catalog_binding(
                typed,
                machine,
                clause,
                selected_target,
                target_machine_origins,
            ) else {
                continue;
            };
            let Some(schema) = crate::service_schema::from_typed_operator(context, operator) else {
                continue;
            };
            let target = selected_target.unwrap_or_default().to_owned();
            let provider_type = provider_owner_path(typed, machine).unwrap_or_default();
            let plan_name = satisfies_plan_name(
                &target,
                schema.identity_text(schema.trait_name),
                &provider_type,
            );
            let position = derived_plan_position(
                &mut plans,
                plan_name,
                provider_type,
                provider_type_package_identity,
                target,
                schema.clone(),
                ProviderSchemaDeclaration::BoundaryOperator(operator.symbol),
                provider_type_symbol,
                origin_package_identity,
            );
            debug_assert_eq!(
                plans[position].provenance.schema,
                ProviderSchemaDeclaration::BoundaryOperator(operator.symbol)
            );
            debug_assert_eq!(
                plans[position].provenance.provider_type,
                provider_type_symbol
            );
            let binding = binding.resolve(&mut plans[position].plan.identities);
            let requirement_identity = mint_plan_row_identity(
                &mut plans[position].plan,
                schema.identity_text(schema.methods[0].requirement_identity),
            );
            plans[position].plan.rows.push(ProviderPlanRow {
                method: "realize".to_owned(),
                requirement_identity,
                requirement_lifetime_partition: Vec::new(),
                admitted_same_stack_bytes: 0,
                binding,
            });
            plans[position]
                .provenance
                .row_requirements
                .push(operator.symbol);
            plans[position]
                .provenance
                .row_realizations
                .push(super::ProviderRowRealization::Authored(machine.symbol));
            plans[position]
                .provenance
                .row_target_machine_origins
                .push(target_machine_origin.clone());
            plans[position].provenance.row_placed_accessors.push(None);
        }
    }
    plans
}

/// A provider row binding whose checked-adapter identity still needs to be
/// interned into its owning plan's table. Rows reach their plan after the
/// draft is built, so canonical identity text rides along until the push
/// site resolves it against `plan.identities`.
enum DraftRowBinding {
    /// A binding that carries no plan-local identities.
    Bound(ProviderBinding),
    /// The canonical machine identity text a checked-adapter binding interns
    /// into the owning plan's `identities`.
    CheckedAdapter {
        machine_identity: String,
        machine_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    },
}

impl DraftRowBinding {
    /// The row binding with any deferred identity interned into `identities`.
    fn resolve(&self, identities: &mut ProviderIdentityTable) -> ProviderBinding {
        match self {
            Self::Bound(binding) => binding.clone(),
            Self::CheckedAdapter {
                machine_identity,
                machine_package_identity,
            } => ProviderBinding::CheckedAdapter {
                machine_identity: identities.intern(machine_identity),
                machine_package_identity: *machine_package_identity,
            },
        }
    }
}

/// Specialization preserves satisfaction edges for checking the concrete body,
/// but does not author another provider declaration. Select the template once;
/// exact specialization receipts join its closed bodies after plan selection.
fn is_authored_provider_machine(
    typed: &checked_trees::typed_trees::TypedTrees,
    machine: &checked_trees::typed_trees::machine::Machine,
) -> bool {
    !typed
        .machine_specializations
        .iter()
        .any(|receipt| receipt.instance == machine.symbol)
}

/// The checked-adapter provider binding a machine supplies through its own
/// checked body: the machine's callable identity plus its package identity.
/// Checked bodies and authenticated compiler leaves share one source-kind
/// decision. Build-owned native bindings enter through their own admitted rows;
/// they are never recovered from authored declaration payloads.
fn checked_or_catalog_binding(
    typed: &checked_trees::typed_trees::TypedTrees,
    machine: &checked_trees::typed_trees::machine::Machine,
    clause: &checked_trees::typed_trees::machine::TraitConformance,
    selected_target: Option<&str>,
    target_machine_origins: &[SelectedTargetMachineOrigin],
) -> Option<(DraftRowBinding, Option<SelectedTargetMachineOrigin>)> {
    match machine.supply_mode {
        language_semantics::MachineSupplyMode::CheckedBody if machine.body_is_present => {
            Some((checked_adapter_draft(typed, machine), None))
        }
        language_semantics::MachineSupplyMode::Boundary
        | language_semantics::MachineSupplyMode::CompilerPrimitive
            if !machine.body_is_present =>
        {
            let (binding, origin) = inferred_compiler_leaf_binding(
                typed,
                machine,
                clause,
                selected_target,
                target_machine_origins,
            )?;
            Some((DraftRowBinding::Bound(binding), origin))
        }
        _ => None,
    }
}

fn checked_adapter_draft(
    typed: &checked_trees::typed_trees::TypedTrees,
    machine: &checked_trees::typed_trees::machine::Machine,
) -> DraftRowBinding {
    DraftRowBinding::CheckedAdapter {
        machine_identity: typed
            .machine_callable_identity_text(machine)
            .unwrap_or_default(),
        machine_package_identity: typed.symbols.symbol_package_identity(machine.symbol),
    }
}

/// The position of the derived plan keyed by its name, provider and origin
/// package identities, schema declaration and provider symbol — inserting the
/// empty plan when absent.
#[allow(clippy::too_many_arguments)]
/// Mint one row's exact requirement identity into the plan's identity table.
/// The table is append-only, so rows pushed earlier keep resolving.
fn mint_plan_row_identity(
    plan: &mut ProviderPlan,
    requirement_identity: &str,
) -> ProviderIdentityId {
    plan.identities.intern(requirement_identity)
}

fn derived_plan_position(
    plans: &mut Vec<DerivedProviderPlan>,
    plan_name: String,
    provider_type: String,
    provider_type_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    target: String,
    schema: ServiceSchema,
    schema_declaration: ProviderSchemaDeclaration,
    provider_type_symbol: Option<symbols::SymbolHandle>,
    origin_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
) -> usize {
    plans
        .iter()
        .position(|derived| {
            derived.plan.name == plan_name
                && derived.plan.provider_type_package_identity == provider_type_package_identity
                && derived.plan.origin_package_identity == origin_package_identity
                && derived.provenance.schema == schema_declaration
                && derived.provenance.provider_type == provider_type_symbol
        })
        .unwrap_or_else(|| {
            plans.push(DerivedProviderPlan {
                plan: ProviderPlan {
                    name: plan_name,
                    provider_application: Default::default(),
                    provider_type,
                    provider_type_package_identity,
                    target,
                    identities: ProviderIdentityTable::default(),
                    schema,
                    rows: Vec::new(),
                    origin_package_identity,
                    origin_package: String::new(),
                    digest_memo: Default::default(),
                },
                provenance: ProviderPlanProvenance {
                    schema: schema_declaration,
                    provider_type: provider_type_symbol,
                    row_requirements: Vec::new(),
                    row_realizations: Vec::new(),
                    row_target_machine_origins: Vec::new(),
                    row_placed_accessors: Vec::new(),
                },
            });
            plans.len() - 1
        })
}

/// Compiler-owned semantic primitives use the same candidate plan/selection
/// route as authored providers, with a separate exact declaration-custody role.
fn derive_placed_accessor_plans(
    context: ProviderPlanningContext<'_>,
    selected_target: Option<&str>,
    plans: &mut Vec<DerivedProviderPlan>,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let Some(target) = selected_target else {
        return Ok(());
    };
    let typed = context.typed;
    for view in &typed.placed_view_plans {
        for field in &view.fields {
            for operation in &field.accessor_targets {
                if !context.contains(operation.machine_symbol) {
                    continue;
                }
                let Some(accessor) =
                    crate::SealedPlacedAccessor::authenticate(context, operation.machine_symbol)
                        .map_err(|error| vec![error])?
                else {
                    continue;
                };
                let schema = crate::service_schema::from_sealed_placed_accessor(context, &accessor)
                    .map_err(|error| vec![error])?;
                let machine = typed.machine(accessor.authored()).ok_or_else(|| {
                    vec![diagnostics::Diagnostic::error(
                        "placed catalog lost its generated machine",
                    )]
                })?;
                let requirement =
                    typed
                        .machine_callable_identity_text(machine)
                        .ok_or_else(|| {
                            vec![diagnostics::Diagnostic::error(
                                "placed catalog lost its exact callable identity",
                            )]
                        })?;
                let provider = typed
                    .normalized_hermetic_symbol_identity(field.accessor_data_symbol)
                    .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
                let position = derived_plan_position(
                    plans,
                    satisfies_plan_name(target, &requirement, &provider),
                    provider,
                    typed
                        .symbols
                        .symbol_package_identity(field.accessor_data_symbol),
                    target.to_owned(),
                    schema,
                    ProviderSchemaDeclaration::SealedPlacedAccessor(accessor.authored()),
                    Some(field.accessor_data_symbol),
                    typed.symbols.symbol_package_identity(machine.symbol),
                );
                let requirement_identity =
                    mint_plan_row_identity(&mut plans[position].plan, &requirement);
                plans[position].plan.rows.push(ProviderPlanRow {
                    method: operation.operation.clone(),
                    requirement_identity,
                    requirement_lifetime_partition: Vec::new(),
                    admitted_same_stack_bytes: 0,
                    binding: ProviderBinding::CompilerIntrinsic {
                        machine: accessor
                            .binding_identity(context)
                            .map_err(|error| vec![error])?,
                    },
                });
                let provenance = &mut plans[position].provenance;
                provenance.row_requirements.push(machine.symbol);
                provenance
                    .row_realizations
                    .push(super::ProviderRowRealization::Authored(machine.symbol));
                provenance.row_target_machine_origins.push(None);
                provenance.row_placed_accessors.push(Some(accessor));
            }
        }
    }
    Ok(())
}
