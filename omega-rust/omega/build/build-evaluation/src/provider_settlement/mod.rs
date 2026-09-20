//! Provider settlement for the checked program: plans derived, validated
//! and selected with provenance over the settled target machines, with every
//! `Independent` selection closed against the build's verified components.
//!
//! `settle_checked_providers` is the route. `independent_components` owns the
//! one subordinate admission: re-verifying the component descriptions the
//! package inputs attached before provider planning joins them.

mod canonical_filesystem_host;
mod independent_components;

pub use independent_components::verify_independent_component_descriptions;

use crate::admission::target_machines::SelectedTargetMachineDeclarations;
use diagnostics::Diagnostic;
use effects::SelectedProviderPlanFacts;
use effects::provider_plan::ProviderPlan;
use package_compilation::PackageCompilationInputs;
use provider_planning::ProviderSelection;
use provider_planning::SelectedProviderReviewProvenance;
use provider_planning::calling_policy_plans::BoundaryCallingPlanRealization;
use provider_planning::derive_satisfies_plans;
use provider_planning::evaluated_via_bindings::EvaluatedViaBindingTable;
use typed_trees::TypedTrees;

/// Final typed provider choices and their evidence, retained for checking and
/// selected execution. Candidate plans stay separate from selected plan facts.
pub struct CheckedProviderSelection {
    pub provider_plans: Vec<ProviderPlan>,
    pub evaluated_via_bindings: EvaluatedViaBindingTable,
    pub selected_provider_plan_facts: SelectedProviderPlanFacts,
    pub selected_provider_provenance: Vec<SelectedProviderReviewProvenance>,
    pub external_binding_rows: Vec<calling_conventions::ExternalBindingRow>,
}

/// Settle the final authored/generated target roster, validate and select its
/// provider candidates, then bind fused erasure and exact invocation evidence.
/// This must precede deferred const evaluation and typed-to-checked settlement.
pub fn settle_checked_providers(
    typed: &mut TypedTrees,
    selected_target_machine_declarations: SelectedTargetMachineDeclarations,
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
    build_provider_selections: &[ProviderSelection],
    accepted_component_assumptions: &std::collections::BTreeSet<[u8; 32]>,
    boundary_calling_plan_realizations: &[BoundaryCallingPlanRealization],
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
) -> Result<CheckedProviderSelection, Vec<Diagnostic>> {
    let target_name = selected_target_profile.map(target::TargetProfile::target_name);
    let provider_selection_target = selected_target_profile
        .map(target::TargetProfile::native_target)
        .unwrap_or_else(target::NativeTarget::host);
    let settled_target_machines =
        selected_target_machine_declarations.settle_provider_defaults(typed)?;
    let target_provider_defaults = settled_target_machines.provider_defaults;
    // PRV4 provider selection mirrors the native pipeline: candidates remain
    // separate by provider type and only the uniquely covering candidate may
    // rewrite adapter calls in the interpreter program.
    let evaluated_via_bindings = provider_planning::evaluated_via_bindings::evaluate_via_bindings(
        typed,
        selected_target_profile,
        package_inputs,
    )?;
    let derived_provider_plans = provider_planning::ProviderPlanDerivation::evaluated(
        typed,
        target_name,
        &evaluated_via_bindings,
        &settled_target_machines.origins,
    )
    .map(|derivation| derive_satisfies_plans(typed, derivation))?;
    let mut provider_plans = derived_provider_plans
        .iter()
        .map(|derived| derived.plan.clone())
        .collect::<Vec<_>>();
    let diagnostics = provider_planning::validate_derived_provider_plan_candidates(
        typed,
        &evaluated_via_bindings,
        &derived_provider_plans,
    );
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let selected_provider_plans = provider_planning::select_derived_provider_plans(
        &derived_provider_plans,
        provider_selection_target,
        &target_provider_defaults,
        build_provider_selections,
    )?;
    // The canonical `FilesystemHost` slot admits no authored conformance or
    // selection row; the toolchain mints the per-target realization plan
    // itself. It joins extraction, fused-service erasure, closure facts, the
    // retained candidate inventory, and the post-derivation review provenance
    // below without entering the pre-selection derivation provenance.
    let toolchain_filesystem_plan = canonical_filesystem_host::mint_canonical_filesystem_host_plan(
        typed,
        package_inputs,
        target_name,
        &selected_provider_plans
            .iter()
            .map(|selected| selected.derived.plan.clone())
            .collect::<Vec<_>>(),
    )?;
    let mut fused_service_erasures = Vec::new();
    if let Some(minted) = &toolchain_filesystem_plan {
        fused_service_erasures.push(typed_trees::typed_trees::FusedServiceErasureAuthorization {
            requirement: minted.trait_symbol,
            provider_plan_digest: *minted.plan.identity_digest().as_bytes(),
        });
    }
    for selected in &selected_provider_plans {
        let composition_mode = selected
            .selected_by
            .composition_mode()
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        if composition_mode != provider_planning::CompositionMode::Fused {
            continue;
        }
        let requirement = selected.derived.provenance.schema.symbol();
        if typed
            .traits()
            .iter()
            .any(|definition| definition.is_boundary && definition.symbol == requirement)
        {
            fused_service_erasures.push(
                typed_trees::typed_trees::FusedServiceErasureAuthorization {
                    requirement,
                    provider_plan_digest: *selected.derived.plan.identity_digest().as_bytes(),
                },
            );
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
    provider_planning::validate_selected_synchronous_invocation_cycles(
        typed,
        &selected_semantic_plans,
    )?;
    if let Some(minted) = &toolchain_filesystem_plan {
        selected_semantic_plans.push(minted.plan.clone());
    }
    let external_binding_rows = provider_planning::extract_native_external_binding_rows(
        target_name,
        provider_selection_target,
        &selected_semantic_plans,
        boundary_calling_plan_realizations,
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
        provider_planning::selected_provider_plan_facts_with_independent_components(
            typed,
            &evaluated_via_bindings,
            selected_provider_plans,
            &independent_components,
        )?;
    if let Some(minted) = toolchain_filesystem_plan {
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
                    "toolchain-settled `FilesystemHost` plan missing from selected facts after join",
                )]
            })?;
        selected_provider_provenance.insert(
            index,
            SelectedProviderReviewProvenance {
                plan: minted.plan.clone(),
                provider: provider_planning::ProviderPlanProvenance {
                    schema: provider_planning::ProviderSchemaDeclaration::BoundaryTrait(
                        minted.trait_symbol,
                    ),
                    provider_type: None,
                    row_requirements: minted.requirement_symbols,
                    row_realizations: vec![
                        symbols::SymbolHandle::invalid();
                        minted.plan.rows.len()
                    ],
                    row_target_machine_origins: vec![None; minted.plan.rows.len()],
                },
                selected_by:
                    provider_planning::ProviderSelectionProvenance::UniqueCoveringCandidate,
                row_compiler_intrinsic_executions: Vec::new(),
            },
        );
        // The trust report replays every selected plan against exactly one
        // retained candidate; the toolchain-settled plan is a real retained
        // candidate for the slot even though no package authored it.
        provider_plans.push(minted.plan);
    }
    Ok(CheckedProviderSelection {
        provider_plans,
        evaluated_via_bindings,
        selected_provider_plan_facts,
        selected_provider_provenance,
        external_binding_rows,
    })
}
