//! Choose which implementation each API uses for the target, and record why
//! each choice was made. Implementations built as separate components
//! (`Independent`) must match verified components attached to this build.
//!
//! `settle_checked_providers` coordinates the work. `independent_components`
//! verifies attached component descriptions before matching them to choices.
//! Candidates are validated when constructed; do not repeat those checks here.
//! The source checker ensures adapters satisfy the API's requirements.
//!
//! Recognizing an API as a filesystem or other service must not invent code
//! that implements it. Use the implementations declared by the program and
//! selected by its build settings. Explicit links between APIs and verification
//! of independently compiled components still follow their usual checks.

pub(crate) mod assumed_bindings;
pub(crate) mod completion;
mod composition_modes;
mod independent_components;
pub(crate) mod semantic_roles;

pub use composition_modes::verify_selected_compositions_are_realized;

use crate::ProviderSelection;
use crate::SelectedProviderReviewProvenance;
use crate::calling_policy_plans::BoundaryCallingPlanRealization;
use crate::evaluated_via_bindings::EvaluatedViaBindingTable;
use diagnostics::Diagnostic;
use effects::SelectedProviderPlanFacts;
use effects::provider_plan::ProviderPlan;
use package_compilation::PackageCompilationInputs;

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
    pub settlement: ProviderSettlement<crate::DeferredProviderRows>,
    pub selected_provider_plan_facts: SelectedProviderPlanFacts,
    pub selected_provider_provenance: Vec<SelectedProviderReviewProvenance>,
    pub selected_declarations:
        Vec<Result<language_semantics::provider_declarations::ProviderDeclarations, Diagnostic>>,
    pub native_provider_sources:
        Result<std::sync::Arc<backend_plan::NativeProviderSources>, Vec<Diagnostic>>,
}

/// Provider-planning results a checked compilation retains for package review,
/// trust replay and native realization. Candidate plans stay separate from the
/// selected plan facts that selected execution owns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSettlement<Rows = crate::PreparedProviderRows> {
    /// Shared Product artifact and exact calling declarations. Retention grants
    /// neither Source formation nor provider execution authority.
    pub calling_program: std::sync::Arc<crate::calling_policy_plans::CallingProgram>,
    pub selected_provider_publication:
        std::sync::Arc<provider_selection::SelectedTargetProviderPublication>,
    pub boundary_provider_demands: Vec<ProviderPlan>,
    /// Every selected implementation, including external operations. Callback
    /// generation and native emission use this same complete list.
    pub provider_declarations: std::sync::Arc<[backend_plan::TerminalTargetProviderPlan]>,
    /// Source-closed authored choices, separate from the actual selected
    /// native-augmented receipt. Publication borrows no Source context.
    pub provider_publication: crate::target_machines::ProviderPublicationCatalog,
    /// Exact target used to derive candidate plans, including semantic-only
    /// checking's supported host profile. This grants no deployment entry.
    pub planning_target_profile: Option<target::TargetProfile>,
    /// Validated boundary calling-plan realizations, computed while typed
    /// declarations and selected opaque representations coexist and closed
    /// over outbound callbacks by checking. Presence publishes no package ABI.
    pub boundary_calling_plan_realizations: Vec<BoundaryCallingPlanRealization>,
    /// Complete applications for explicitly Independent checked boundaries.
    /// These are neither foreign binding rows nor fused internal-call ABIs.
    pub component_calling_applications: Vec<calling_conventions::BoundaryCallingApplication>,
    /// Every possible implementation. Package review checks that each selected
    /// implementation matches exactly one entry in this list.
    pub provider_candidates: Vec<crate::ProviderPlanCandidate<Rows>>,
    /// Complete authored native candidates, including private/unselected rows.
    /// Package output and audit use this inventory, never runtime demand as an
    /// excuse to discard a declared foreign promise.
    pub native_build_plans: Vec<crate::DerivedProviderPlan>,
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
    /// Original consumer verdicts, in the same package order as descriptions.
    /// Native composition borrows the verified artifact and export handles;
    /// it must not rebuild a proof envelope or infer installation authority.
    pub verified_independent_components:
        Vec<std::sync::Arc<component_description::VerifiedComponent>>,
}

impl ProviderSettlement<crate::DeferredProviderRows> {
    /// Report saved declaration errors after program-entry checks and before
    /// completing direct calls. Preserve error order without reading source.
    pub fn finish_row_declarations(self) -> Result<ProviderSettlement, Vec<Diagnostic>> {
        let Self {
            calling_program,
            selected_provider_publication,
            boundary_provider_demands,
            provider_declarations,
            provider_publication,
            planning_target_profile,
            boundary_calling_plan_realizations,
            component_calling_applications,
            provider_candidates,
            native_build_plans,
            evaluated_via_bindings,
            external_binding_rows,
            independent_component_descriptions,
            verified_independent_components,
        } = self;
        let mut prepared = Vec::with_capacity(provider_candidates.len());
        for candidate in provider_candidates {
            prepared.push(
                candidate
                    .finish_row_declarations()
                    .map_err(|diagnostic| vec![diagnostic])?,
            );
        }
        Ok(ProviderSettlement {
            calling_program,
            selected_provider_publication,
            boundary_provider_demands,
            provider_declarations,
            provider_publication,
            planning_target_profile,
            boundary_calling_plan_realizations,
            component_calling_applications,
            provider_candidates: prepared,
            native_build_plans,
            evaluated_via_bindings,
            external_binding_rows,
            independent_component_descriptions,
            verified_independent_components,
        })
    }
}

impl<Intrinsic> ProviderSettlement<Intrinsic> {
    /// Publish the complete calling catalog from its producing owners. Explicit
    /// policy rows can also own an external import; retain that same application
    /// once. Default native policies retain their materialized geometry here too.
    /// Export applications arrive from component preparation; merging them
    /// requires no source lookup or policy execution.
    pub fn calling_applications(
        &self,
        exported: &[calling_conventions::BoundaryCallingApplication],
    ) -> Result<Vec<calling_conventions::BoundaryCallingApplication>, String> {
        let explicit = self
            .boundary_calling_plan_realizations
            .iter()
            .map(BoundaryCallingPlanRealization::native_application)
            .collect::<Result<Vec<_>, _>>()?;
        let mut applications = Vec::with_capacity(explicit.len());
        for application in explicit {
            if !applications.contains(&application) {
                applications.push(application);
            }
        }
        for application in self
            .external_binding_rows
            .iter()
            .flat_map(|row| &row.calling_applications)
        {
            // A declaration ABI can have several authored policy realizations.
            // Retain each complete value; actual consumers join the selected
            // schema's application commitment as well as its full ABI key.
            if !applications.contains(application) {
                applications.push(application.clone());
            }
        }
        for application in self.component_calling_applications.iter().chain(exported) {
            if !applications.contains(application) {
                applications.push(application.clone());
            }
        }
        Ok(applications)
    }
}

/// Derive every candidate, selecting whole maps only for demanded or authored
/// slots. Unselected exported implementations remain checked review content.
pub fn settle_checked_providers(
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
    prepared_providers: Result<crate::target_machines::PreparedProductProviders, Vec<Diagnostic>>,
    accepted_component_assumptions: &std::collections::BTreeSet<[u8; 32]>,
    boundary_calling_plan_realizations: Vec<BoundaryCallingPlanRealization>,
    calling_program: std::sync::Arc<crate::calling_policy_plans::CallingProgram>,
) -> Result<CheckedProviderSelection, Vec<Diagnostic>> {
    // Every Product body has already been checked. This target chooses only
    // realization rows; it never narrows the source program.
    let planning_target = selected_target_profile.or_else(target::TargetProfile::host_if_supported);
    let target_name = planning_target.map(target::TargetProfile::target_name);
    let provider_selection_target = planning_target
        .map(target::TargetProfile::native_target)
        .unwrap_or_else(target::NativeTarget::host);
    // PRV4 provider selection mirrors the native pipeline: candidates remain
    // separate by provider type and only the uniquely covering candidate may
    // rewrite adapter calls in the interpreter program.
    let prepared = prepared_providers?.attach_calling(
        &calling_program,
        planning_target,
        &boundary_calling_plan_realizations,
    )?;
    let evaluated_via_bindings = prepared.bindings;
    let product_candidates = prepared.candidates;
    let native_build_plans = product_candidates[prepared.native_start..]
        .iter()
        .map(|candidate| candidate.source.clone())
        .collect();
    let provider_publication = prepared.publication;
    let completion_roles = prepared.completion_roles;
    let independent_components = independent_components::verify_independent_components(
        package_inputs,
        accepted_component_assumptions,
    )?
    .into_iter()
    .map(std::sync::Arc::new)
    .collect::<Vec<_>>();
    let prepared_assumptions = prepared.assumptions;
    let assumptions = prepared_assumptions.coordinates?;
    let profile = planning_target.unwrap_or_else(target::TargetProfile::host);
    let mut prepared = prepared.choices?;
    prepared.independent_components = independent_components.clone();
    let artifact = calling_program.artifact.view();
    let canonical_roots = prepared.close_application_roots(artifact)?;
    let provider_selection::SelectedTargetProviders {
        selection: canonical_selection,
        facts: mut selected_provider_plan_facts,
        demanded_plans: boundary_provider_demands,
        choices,
        invoked_declarations: _,
    } = prepared.select(
        artifact,
        provider_selection::ProviderApplicationDemand::Product(canonical_roots.as_deref()),
        &assumptions,
    )?;
    let provider_declarations: std::sync::Arc<[backend_plan::TerminalTargetProviderPlan]> =
        canonical_selection.plans.clone().into();
    let selected_provider_plans = choices;
    let direct_context = crate::provider_planning::external_binding_rows::ExternalBindingContext {
        calling_program: &calling_program,
        selected_target: target_name,
        native_target: provider_selection_target,
        boundary_calling_plan_realizations: &boundary_calling_plan_realizations,
    };
    let provider_candidates = product_candidates
        .into_iter()
        .map(|candidate| {
            candidate
                .rows
                .attach_physical_rows(candidate.source, &direct_context)
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Check every declared closed instance, including uncalled candidates.
    backend_plan::synchronous_invocations::validate_declared_synchronous_invocation_cycles(
        &provider_declarations,
        artifact.module(),
    )?;
    let external_binding_rows = crate::extract_native_external_binding_rows(
        target_name,
        provider_selection_target,
        &selected_provider_plans
            .iter()
            .map(|selected| &selected.candidate.plan)
            .collect::<Vec<_>>(),
        &boundary_calling_plan_realizations,
        &calling_program,
    )?;
    // Only a selected implementation must keep its promise to exit. Report
    // its saved result here, after the same binding and cycle checks as before.
    let mut selected_declarations = Vec::with_capacity(selected_provider_plans.len());
    let mut selected_families = Vec::with_capacity(selected_provider_plans.len());
    let completed_providers = selected_provider_plans
        .into_iter()
        .map(|selected| {
            let review = selected.candidate.owner;
            selected_declarations.push(review.declarations);
            selected_families.push(review.families);
            Ok((
                SelectedProviderReviewProvenance {
                    plan: selected.candidate.plan,
                    provider: review.provenance,
                    selected_by:
                        crate::provider_planning::selection_declarations::restore_provenance(
                            selected.selected_by,
                        ),
                    row_compiler_intrinsic_executions: Vec::new(),
                    row_external_completion_leaves: review.completion?,
                },
                review.role,
            ))
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    if package_inputs.is_some_and(PackageCompilationInputs::semantic_role_discovery) {
        completion_roles.validate(&completed_providers)?;
    }
    let selected_provider_provenance = completed_providers
        .into_iter()
        .map(|(provider, _)| provider)
        .collect::<Vec<_>>();
    let native_provider_sources = crate::native_provider_sources::attach_native_provider_sources(
        &selected_provider_provenance,
        selected_families,
        std::sync::Arc::clone(&provider_declarations),
    )
    .map(std::sync::Arc::new);
    for assumption in prepared_assumptions.rows {
        assumption.validate_calling(&boundary_calling_plan_realizations)?;
    }
    let mut required = std::collections::BTreeSet::new();
    for row in &assumptions {
        if !required.insert(row.required.as_str())
            || selected_provider_plan_facts
                .selected_requirement_row(&row.required)
                .is_some()
        {
            return Err(vec![Diagnostic::error(
                "assumed binding has competing explicit supply",
            )]);
        }
    }
    selected_provider_plan_facts = selected_provider_plan_facts.with_assumed_bindings(assumptions);
    for row in selected_provider_plan_facts.assumed_bindings() {
        let supplied = selected_provider_plan_facts
            .assumed_supplier_identity(&row.required)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        if selected_provider_plan_facts
            .selected_requirement_row(supplied)
            .is_none()
        {
            return Err(vec![Diagnostic::error(
                "assumed binding has no selected complete supplied provider",
            )]);
        }
    }
    let mut component_providers = Vec::new();
    for selected in &selected_provider_provenance {
        if selected
            .selected_by
            .composition_mode()
            .map_err(|reason| vec![Diagnostic::error(reason)])?
            == crate::CompositionMode::Independent
        {
            component_providers.push(&selected.plan);
        }
    }
    let component_calling_applications =
        crate::calling_policy_plans::component_calling_applications(
            planning_target.unwrap_or_else(target::TargetProfile::host),
            &calling_program,
            &component_providers,
            &boundary_calling_plan_realizations,
        )?;
    let selected_provider_publication =
        std::sync::Arc::new(provider_selection::SelectedTargetProviderPublication {
            profile,
            terminal_psi: calling_program.artifact.view().identity(),
            application_roots: canonical_roots,
            selection: canonical_selection,
        });
    Ok(CheckedProviderSelection {
        settlement: ProviderSettlement {
            calling_program,
            selected_provider_publication,
            boundary_provider_demands,
            provider_declarations,
            provider_publication,
            planning_target_profile: planning_target,
            boundary_calling_plan_realizations,
            component_calling_applications,
            provider_candidates,
            native_build_plans,
            evaluated_via_bindings,
            external_binding_rows,
            verified_independent_components: independent_components,
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
        selected_declarations,
        native_provider_sources,
    })
}
