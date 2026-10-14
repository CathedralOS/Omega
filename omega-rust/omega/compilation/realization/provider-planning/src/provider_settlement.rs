//! Provider settlement for the checked program: plans derived and selected
//! with provenance over the settled target machines, with every
//! `Independent` selection closed against the build's verified components.
//! Derivation is the one producer of each candidate; settlement does not
//! re-validate what it just derived, and adapter refinement of requirement
//! ceilings belongs to Psi conformance checking.
//!
//! `settle_checked_providers` is the route. `independent_components` owns the
//! one subordinate admission: re-verifying the component descriptions the
//! package inputs attached before provider planning joins them.
//!
//! The canonical `FilesystemHost` slot may use a fallback toolchain plan when
//! its accepted schema has no selected authored provider. TimeHost is supplied
//! entirely by ordinary target-scoped std providers.
//! `Console` is deliberately absent from the fallback set:
//! its canonical boundary is authored-provider-complete — every hosted
//! target ships a `ConsoleNativeProvider` conformance plus a
//! `provider_defaults` selection in `targets/<target>/console_impl.omg`, and
//! its leaves close through intrinsic review, syscall rows, or DLL imports
//! under accepted bindings. A program whose `Console` boundary selects no
//! provider is missing program content, not a settlement slot; provider
//! identity is nominal and never inferred from the boundary's name.

mod canonical_filesystem_host;
mod composition_modes;
mod independent_components;
mod semantic_roles;

pub use composition_modes::verify_selected_compositions_are_realized;

use crate::ProviderSelection;
use crate::SelectedProviderReviewProvenance;
use crate::calling_policy_plans::BoundaryCallingPlanRealization;
use crate::derive_satisfies_plans;
use crate::evaluated_via_bindings::EvaluatedViaBindingTable;
use checked_trees::typed_trees::TypedTrees;
use diagnostics::Diagnostic;
use effects::SelectedProviderPlanFacts;
use effects::provider_plan::{
    ProviderBinding, ProviderIdentityTable, ProviderPlan, ProviderPlanRow,
};
use package_compilation::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, PackageCompilationInputs,
};

/// Resolve the exact Product boundary and normalized schema for the canonical
/// filesystem host binding.
fn resolve_canonical_host_binding<'trees>(
    typed: &'trees TypedTrees,
    calling_plans: &[BoundaryCallingPlanRealization],
    binding: &AcceptedSemanticBinding,
) -> Result<
    (
        &'trees checked_trees::typed_trees::trait_definition::TraitDefinition,
        effects::provider_plan::ServiceSchema,
    ),
    Diagnostic,
> {
    let matches = typed
        .traits()
        .iter()
        .filter_map(|definition| {
            if !definition.is_boundary
                || typed.symbols.symbol_package_identity(definition.symbol)
                    != Some(binding.package())
                || typed.symbols.display_path(definition.symbol, "::") != binding.declaration_path()
            {
                return None;
            }
            let schema = crate::service_schema::from_typed(
                crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product),
                calling_plans,
                definition,
            )?;
            (schema.trait_package_identity == Some(binding.package())
                && package_compilation::accepted_service_schema_digest(binding.role(), &schema)
                    == binding.normalized_schema_digest())
            .then_some((definition, schema))
        })
        .collect::<Vec<_>>();
    let [resolved] = matches.as_slice() else {
        return Err(Diagnostic::error(format!(
            "accepted semantic binding {:?} resolved to {} exact package-owned boundary declarations instead of one",
            binding.role(),
            matches.len(),
        )));
    };
    Ok(resolved.clone())
}

/// The toolchain-settled mint output shared by the canonical host plans:
/// the plan plus the exact symbols the provenance and erasure joins need.
pub(crate) struct ToolchainSettledMint {
    pub plan: ProviderPlan,
    pub trait_symbol: symbols::SymbolHandle,
    /// Row-aligned with `plan.rows`: the requirement's own machine symbol.
    /// Realization symbols deliberately stay `invalid` downstream — the
    /// toolchain settlement table, not an authored machine, realizes the row.
    pub requirement_symbols: Vec<symbols::SymbolHandle>,
}

/// One canonical host requirement realized by one positional kernel syscall
/// on the selected target.
pub(super) struct ToolchainSettledLeaf {
    /// Canonical requirement name; locates the method inside the accepted
    /// service schema. It is never an identity key in the emitted plan — the
    /// row carries the method's complete normalized requirement identity.
    pub method: &'static str,
    /// Target syscall number for the selected target profile.
    pub number: i64,
}

/// Mint one canonical host's toolchain-settled plan. `leaves` is the host's
/// reviewed realization table for the selected target (empty when the target
/// has none); each leaf names the schema method it satisfies and the syscall
/// that realizes it. `provider` and `host_label` supply the plan's nominal
/// identity and diagnostic label.
fn mint_toolchain_settled_plan(
    typed: &TypedTrees,
    calling_plans: &[BoundaryCallingPlanRealization],
    accepted_binding: Option<&AcceptedSemanticBinding>,
    target_name: Option<&'static str>,
    already_selected: &[ProviderPlan],
    leaves: &[ToolchainSettledLeaf],
    provider: &'static str,
    host_label: &'static str,
) -> Result<Option<ToolchainSettledMint>, Vec<Diagnostic>> {
    let Some(binding) = accepted_binding else {
        return Ok(None);
    };
    let Some(target_name) = target_name else {
        return Ok(None);
    };
    if leaves.is_empty() {
        return Ok(None);
    }
    let (definition, schema) = resolve_canonical_host_binding(typed, calling_plans, binding)
        .map_err(|error| vec![error])?;
    let signatures = typed.trait_machine_signatures(definition);
    if already_selected.iter().any(|selected| {
        selected.schema.trait_package_identity == schema.trait_package_identity
            && selected.schema.trait_name == schema.trait_name
    }) {
        return Ok(None);
    }
    let mut rows = Vec::with_capacity(leaves.len());
    let mut requirement_symbols = Vec::with_capacity(leaves.len());
    let mut identities = ProviderIdentityTable::default();
    for leaf in leaves {
        let Some(method) = schema
            .methods
            .iter()
            .find(|method| method.name == leaf.method)
        else {
            continue;
        };
        let Some(signature) = signatures
            .iter()
            .find(|signature| signature.name.as_str() == method.name)
        else {
            return Err(vec![Diagnostic::error(format!(
                "canonical `{host_label}` requirement `{}` has no retained typed requirement signature",
                leaf.method,
            ))]);
        };
        requirement_symbols.push(signature.symbol);
        rows.push(ProviderPlanRow {
            method: method.name.clone(),
            requirement_identity: identities
                .intern(schema.identity_text(method.requirement_identity)),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::Syscall {
                number: leaf.number,
            },
        });
    }
    if rows.is_empty() {
        // Nothing the schema demands has a toolchain-settled realization:
        // no plan mints, the same as an unreviewed realization table.
        return Ok(None);
    }
    let plan = ProviderPlan {
        name: crate::satisfies_plan_name(
            target_name,
            schema.identity_text(schema.trait_name),
            provider,
        ),
        provider_type: provider.to_owned(),
        provider_type_package_identity: None,
        target: target_name.to_owned(),
        schema,
        rows,
        identities,
        origin_package_identity: None,
        origin_package: "omega toolchain".to_owned(),
        digest_memo: Default::default(),
    };
    let errors = plan.validate_candidate_against_schema();
    if !errors.is_empty() {
        return Err(vec![Diagnostic::error(format!(
            "toolchain-settled `{host_label}` plan is malformed: {}",
            errors.join("; ")
        ))]);
    }
    Ok(Some(ToolchainSettledMint {
        plan,
        trait_symbol: definition.symbol,
        requirement_symbols,
    }))
}

/// Target defaults and declaration origins prepared before provider planning.
/// Build admission resolves its retained declarations to these exact typed
/// identities; settlement consumes them without depending on source filtering.
pub struct SettledTargetMachineDeclarations {
    pub provider_defaults: Vec<ProviderSelection>,
    pub origins: Vec<crate::SelectedTargetMachineOrigin>,
}

/// Final typed provider choices and their evidence. The selected plan facts
/// and review provenance move into checking and selected execution; the
/// settlement stays provider-planning custody beside the checked program.
pub struct CheckedProviderSelection {
    pub settlement: ProviderSettlement,
    pub selected_provider_plan_facts: SelectedProviderPlanFacts,
    pub selected_provider_provenance: Vec<SelectedProviderReviewProvenance>,
}

/// Provider-planning results a checked compilation retains for package review,
/// trust replay and native realization. Candidate plans stay separate from the
/// selected plan facts that selected execution owns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSettlement {
    /// Exact target used to derive candidate plans, including semantic-only
    /// checking's supported host profile. This grants no deployment entry.
    pub planning_target_profile: Option<target::TargetProfile>,
    /// Validated boundary calling-plan realizations, computed while typed
    /// declarations and selected opaque representations coexist and closed
    /// over outbound callbacks by checking. Presence publishes no package ABI.
    pub boundary_calling_plan_realizations: Vec<BoundaryCallingPlanRealization>,
    /// Every derived candidate plan plus toolchain-settled plans; trust
    /// replays each selected plan against exactly one of these.
    pub provider_plans: Vec<ProviderPlan>,
    /// Complete ordinary-`via` evaluation population, including leaves not
    /// selected for this executable. Backend lowering reads only the imports
    /// installed in provider plans.
    pub evaluated_via_bindings: EvaluatedViaBindingTable,
    /// Selected normalized-import bindings and their evaluated calling plans,
    /// the only normalized foreign-locator custody in native realization.
    pub external_binding_rows: Vec<calling_conventions::ExternalBindingRow>,
    /// The attached component descriptions this settlement verified, in
    /// package-identity order, so a replay re-verifies the same admission
    /// instead of rediscovering unattached inputs.
    pub independent_component_descriptions:
        Vec<package_compilation::IndependentComponentDescription>,
}

/// Derive and select providers against the immutable, source-checked roster.
/// A realization cannot rewrite source contracts or fold target values into them.
pub fn settle_checked_providers(
    typed: &TypedTrees,
    settled_target_machines: SettledTargetMachineDeclarations,
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
    build_provider_selections: &[ProviderSelection],
    accepted_component_assumptions: &std::collections::BTreeSet<[u8; 32]>,
    boundary_calling_plan_realizations: Vec<BoundaryCallingPlanRealization>,
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
    synchronous_invocations: &checked_trees::SynchronousInvocationFacts,
) -> Result<CheckedProviderSelection, Vec<Diagnostic>> {
    // Every Product body has already been checked. This target chooses only
    // realization rows; it never narrows the source program.
    let planning_target = selected_target_profile.or_else(target::TargetProfile::host_if_supported);
    let target_name = planning_target.map(target::TargetProfile::target_name);
    let provider_selection_target = planning_target
        .map(target::TargetProfile::native_target)
        .unwrap_or_else(target::NativeTarget::host);
    let target_provider_defaults = settled_target_machines.provider_defaults;
    let context = crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product);
    // PRV4 provider selection mirrors the native pipeline: candidates remain
    // separate by provider type and only the uniquely covering candidate may
    // rewrite adapter calls in the interpreter program.
    let evaluated_via_bindings = crate::evaluated_via_bindings::evaluate_via_bindings(
        context,
        planning_target,
        package_inputs,
    )?;
    let derived_provider_plans = derive_satisfies_plans(crate::ProviderPlanDerivation::evaluated(
        context,
        &boundary_calling_plan_realizations,
        target_name,
        &evaluated_via_bindings,
        &settled_target_machines.origins,
    ))?;
    let selected_provider_plans = crate::select_derived_provider_plans(
        &derived_provider_plans,
        provider_selection_target,
        &target_provider_defaults,
        build_provider_selections,
    )?;
    if package_inputs.is_some_and(PackageCompilationInputs::semantic_role_discovery) {
        semantic_roles::validate_proposed(typed, &selected_provider_plans)?;
    }
    let mut provider_plans = derived_provider_plans
        .iter()
        .map(|derived| derived.plan.clone())
        .collect::<Vec<_>>();
    // The canonical `FilesystemHost` slot admits no authored conformance or
    // selection row; the toolchain mints the per-target realization plan
    // itself. It joins extraction, fused-service erasure, closure facts, the
    // retained candidate inventory, and the post-derivation review provenance
    // below without entering the pre-selection derivation provenance.
    let toolchain_filesystem_plan = canonical_filesystem_host::mint_canonical_filesystem_host_plan(
        typed,
        &boundary_calling_plan_realizations,
        package_inputs.and_then(|inputs| {
            inputs.accepted_semantic_binding(AcceptedSemanticBindingRole::FilesystemHostService)
        }),
        target_name,
        &selected_provider_plans
            .iter()
            .map(|selected| selected.derived.plan.clone())
            .collect::<Vec<_>>(),
    )?;
    let mut selected_semantic_plans = selected_provider_plans
        .iter()
        .map(|selected| selected.derived.plan.clone())
        .collect::<Vec<_>>();
    // Cycle review only walks authored `invokes` edges and requires every
    // schema method to bind a plan row. The toolchain-settled plan covers
    // only its honestly realizable leaves and its syscall rows can never
    // invoke, so it joins after the authored-plan cycle check, before the
    // external-binding extraction that binds its syscall numbers.
    crate::validate_selected_synchronous_invocation_cycles(
        typed,
        synchronous_invocations,
        &selected_semantic_plans,
    )?;
    if let Some(minted) = &toolchain_filesystem_plan {
        selected_semantic_plans.push(minted.plan.clone());
    }
    let external_binding_rows = crate::extract_native_external_binding_rows(
        target_name,
        provider_selection_target,
        &selected_semantic_plans,
        &boundary_calling_plan_realizations,
        typed,
        opaque_representation_selections,
    )?;
    // Component-closure join. The verified components come only from the
    // descriptions attached to this compilation's package inputs, each
    // re-verified under the build's profile; every `Independent` selection
    // must be realized by exactly one of them and every component must
    // realize one selection, so a build that supplies none, several, an
    // unmatched extra, or a mismatched realization rejects here.
    let independent_components = independent_components::verify_independent_components(
        package_inputs,
        accepted_component_assumptions,
    )?;
    let (mut selected_provider_plan_facts, mut selected_provider_provenance) =
        crate::selected_provider_plan_facts_with_independent_components(
            crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product),
            selected_provider_plans,
            &independent_components,
        )?;
    for minted in [toolchain_filesystem_plan].into_iter().flatten() {
        selected_provider_plan_facts = selected_provider_plan_facts
            .with_toolchain_settled_plan(minted.plan.clone())
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        // Review provenance stays index-aligned with `facts.plans()` (the
        // intrinsic-review join zips them positionally); splice the toolchain
        // row at the same slot the sorted insert landed on. Its realization
        // symbols are `invalid` because the settlement table, not an authored
        // machine, realizes each row.
        let index = selected_provider_plan_facts
            .plans()
            .iter()
            .position(|plan| *plan == minted.plan)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "toolchain-settled canonical-host plan missing from selected facts after join",
                )]
            })?;
        selected_provider_provenance.insert(
            index,
            SelectedProviderReviewProvenance {
                plan: minted.plan.clone(),
                provider: crate::ProviderPlanProvenance {
                    schema: crate::ProviderSchemaDeclaration::BoundaryTrait(minted.trait_symbol),
                    provider_type: None,
                    row_requirements: minted.requirement_symbols,
                    row_realizations: vec![
                        symbols::SymbolHandle::invalid();
                        minted.plan.rows.len()
                    ],
                    row_target_machine_origins: vec![None; minted.plan.rows.len()],
                    row_placed_accessors: vec![None; minted.plan.rows.len()],
                },
                selected_by: crate::ProviderSelectionProvenance::UniqueCoveringCandidate,
                row_compiler_intrinsic_executions: Vec::new(),
            },
        );
        // The trust report replays every selected plan against exactly one
        // retained candidate; the toolchain-settled plan is a real retained
        // candidate for the slot even though no package authored it.
        provider_plans.push(minted.plan);
    }
    Ok(CheckedProviderSelection {
        settlement: ProviderSettlement {
            planning_target_profile: planning_target,
            boundary_calling_plan_realizations,
            provider_plans,
            evaluated_via_bindings,
            external_binding_rows,
            independent_component_descriptions: package_inputs
                .map(|inputs| {
                    inputs
                        .independent_component_descriptions()
                        .cloned()
                        .collect()
                })
                .unwrap_or_default(),
        },
        selected_provider_plan_facts,
        selected_provider_provenance,
    })
}
