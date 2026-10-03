//! Provider settlement for the checked program: plans derived, validated
//! and selected with provenance over the settled target machines, with every
//! `Independent` selection closed against the build's verified components.
//!
//! `settle_checked_providers` is the route. `independent_components` owns the
//! one subordinate admission: re-verifying the component descriptions the
//! package inputs attached before provider planning joins them.
//!
//! Canonical `FilesystemHost` and `TimeHost` slots may use fallback
//! toolchain plans when their accepted schema has no selected authored
//! provider. Ordinary reviewed checked providers take precedence: macOS
//! TimeHost supplies explicit satisfaction and selection in std, including
//! checked calibration bodies and adapters around private imports.
//! `Console` is deliberately absent from the fallback set:
//! its canonical boundary is authored-provider-complete — every hosted
//! target ships a `ConsoleNativeProvider` conformance plus a
//! `provider_defaults` selection in `targets/<target>/console_impl.omg`, and
//! its leaves close through intrinsic review, syscall rows, or DLL imports
//! under accepted bindings. A program whose `Console` boundary selects no
//! provider is missing program content, not a settlement slot; provider
//! identity is nominal and never inferred from the boundary's name.

mod canonical_filesystem_host;
mod canonical_time_host;
mod composition_modes;
mod independent_components;
mod semantic_roles;

pub use canonical_filesystem_host::{
    MintedFilesystemHostPlan, mint_canonical_filesystem_host_plan,
};
pub use canonical_time_host::{MintedTimeHostPlan, mint_canonical_time_host_plan};
pub use composition_modes::verify_selected_compositions_are_realized;
pub use independent_components::verify_independent_component_descriptions;

use crate::ProviderSelection;
use crate::SelectedProviderReviewProvenance;
use crate::calling_policy_plans::BoundaryCallingPlanRealization;
use crate::derive_satisfies_plans;
use crate::evaluated_via_bindings::EvaluatedViaBindingTable;
use diagnostics::Diagnostic;
use effects::SelectedProviderPlanFacts;
use effects::provider_plan::ProviderPlan;
use package_compilation::{AcceptedSemanticBindingRole, PackageCompilationInputs};
use typed_trees::TypedTrees;

/// The toolchain-settled mint output shared by the canonical host plans:
/// the plan plus the exact symbols the provenance and erasure joins need.
struct ToolchainSettledMint {
    plan: ProviderPlan,
    trait_symbol: symbols::SymbolHandle,
    requirement_symbols: Vec<symbols::SymbolHandle>,
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

/// Settle the final authored/generated target roster, validate and select its
/// provider candidates, then bind fused erasure and exact invocation evidence.
/// This must precede deferred const evaluation and typed-to-checked settlement.
pub fn settle_checked_providers(
    typed: &mut TypedTrees,
    settled_target_machines: SettledTargetMachineDeclarations,
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
    build_provider_selections: &[ProviderSelection],
    accepted_component_assumptions: &std::collections::BTreeSet<[u8; 32]>,
    boundary_calling_plan_realizations: Vec<BoundaryCallingPlanRealization>,
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
) -> Result<CheckedProviderSelection, Vec<Diagnostic>> {
    // Target-scoped machines were selected before resolution against the
    // choice `filter_target_machines_by_scope` makes: the requested target,
    // or the compiler host when none is requested. Provider planning follows
    // that same choice, so an unprofiled check derives and evaluates exactly
    // the leaves it selected.
    let planning_target = selected_target_profile.or_else(target::TargetProfile::host_if_supported);
    let target_name = planning_target.map(target::TargetProfile::target_name);
    let provider_selection_target = planning_target
        .map(target::TargetProfile::native_target)
        .unwrap_or_else(target::NativeTarget::host);
    let target_provider_defaults = settled_target_machines.provider_defaults;
    semantic_roles::attach_accepted(typed, package_inputs)?;
    let context = crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product);
    // PRV4 provider selection mirrors the native pipeline: candidates remain
    // separate by provider type and only the uniquely covering candidate may
    // rewrite adapter calls in the interpreter program.
    let evaluated_via_bindings = crate::evaluated_via_bindings::evaluate_via_bindings(
        context,
        planning_target,
        package_inputs,
    )?;
    let derive_selected = |typed: &TypedTrees| {
        let context = crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product);
        let derived = crate::ProviderPlanDerivation::evaluated(
            context,
            target_name,
            &evaluated_via_bindings,
            &settled_target_machines.origins,
        )
        .map(derive_satisfies_plans)?;
        let diagnostics = crate::validate_derived_provider_plan_candidates(
            context,
            &evaluated_via_bindings,
            &derived,
        );
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        let selected = crate::select_derived_provider_plans(
            &derived,
            provider_selection_target,
            &target_provider_defaults,
            build_provider_selections,
        )?;
        Ok((derived, selected))
    };
    let (mut derived_provider_plans, mut selected_provider_plans) = derive_selected(typed)?;
    if package_inputs.is_some_and(PackageCompilationInputs::semantic_role_discovery)
        && semantic_roles::attach_proposed(typed, &selected_provider_plans)?
    {
        // Only completion-dependent provider metadata is rebuilt. Build and
        // ordinary `via` evaluation above execute exactly once.
        (derived_provider_plans, selected_provider_plans) = derive_selected(typed)?;
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
        package_inputs.and_then(|inputs| {
            inputs.accepted_semantic_binding(AcceptedSemanticBindingRole::FilesystemHostService)
        }),
        target_name,
        &selected_provider_plans
            .iter()
            .map(|selected| selected.derived.plan.clone())
            .collect::<Vec<_>>(),
    )?;
    // An already-selected ordinary TimeHost provider keeps its exact
    // provenance. Only an unfilled slot may use the reviewed fallback table.
    let toolchain_time_host_plan = canonical_time_host::mint_canonical_time_host_plan(
        typed,
        package_inputs.and_then(|inputs| {
            inputs.accepted_semantic_binding(AcceptedSemanticBindingRole::TimeHostService)
        }),
        target_name,
        &selected_provider_plans
            .iter()
            .map(|selected| selected.derived.plan.clone())
            .collect::<Vec<_>>(),
    )?;
    // Toolchain-minted host services are always Fused.
    let mut fused_service_erasures = [
        toolchain_filesystem_plan
            .as_ref()
            .map(|minted| minted.trait_symbol),
        toolchain_time_host_plan
            .as_ref()
            .map(|minted| minted.trait_symbol),
    ]
    .into_iter()
    .flatten()
    .map(|requirement| typed_trees::typed_trees::FusedServiceErasureAuthorization { requirement })
    .collect::<Vec<_>>();
    for selected in &selected_provider_plans {
        let composition_mode = selected
            .selected_by
            .composition_mode()
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        if composition_mode != crate::CompositionMode::Fused {
            continue;
        }
        let requirement = selected.derived.provenance.schema.symbol();
        if typed
            .traits()
            .iter()
            .any(|definition| definition.is_boundary && definition.symbol == requirement)
        {
            fused_service_erasures
                .push(typed_trees::typed_trees::FusedServiceErasureAuthorization { requirement });
        }
    }
    typed
        .bind_fused_service_erasures(fused_service_erasures)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let mut selected_semantic_plans = selected_provider_plans
        .iter()
        .map(|selected| selected.derived.plan.clone())
        .collect::<Vec<_>>();
    // Cycle review only walks authored `invokes` edges and requires every
    // schema method to bind a plan row. The toolchain-settled plan covers
    // only its honestly realizable leaves and its syscall rows can never
    // invoke, so it joins after the authored-plan cycle check, before the
    // external-binding extraction that binds its syscall numbers.
    crate::validate_selected_synchronous_invocation_cycles(typed, &selected_semantic_plans)?;
    if let Some(minted) = &toolchain_filesystem_plan {
        selected_semantic_plans.push(minted.plan.clone());
    }
    if let Some(minted) = &toolchain_time_host_plan {
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
            &evaluated_via_bindings,
            selected_provider_plans,
            &independent_components,
        )?;
    for minted in [
        toolchain_filesystem_plan.map(|minted| ToolchainSettledMint {
            plan: minted.plan,
            trait_symbol: minted.trait_symbol,
            requirement_symbols: minted.requirement_symbols,
        }),
        toolchain_time_host_plan.map(|minted| ToolchainSettledMint {
            plan: minted.plan,
            trait_symbol: minted.trait_symbol,
            requirement_symbols: minted.requirement_symbols,
        }),
    ]
    .into_iter()
    .flatten()
    {
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
