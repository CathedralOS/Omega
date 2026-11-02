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
//! target ships a named `NativeConsole` implementation plus a
//! `provider_defaults` selection in `targets/<target>/console_impl.omg`, and
//! its leaves close through intrinsic review, syscall rows, or DLL imports
//! under accepted bindings. A program whose `Console` boundary selects no
//! provider is missing program content, not a settlement slot; provider
//! identity is nominal and never inferred from the boundary's name.

pub(crate) mod assumed_bindings;
mod canonical_filesystem_host;
mod completion;
mod composition_modes;
mod independent_components;
mod semantic_roles;

pub use composition_modes::verify_selected_compositions_are_realized;

use crate::ProviderSelection;
use crate::SelectedProviderReviewProvenance;
use crate::calling_policy_plans::BoundaryCallingPlanRealization;
use crate::evaluated_via_bindings::EvaluatedViaBindingTable;
use crate::provider_planning::trait_name_text;
use checked_trees::typed_trees::service_declarations::ServiceCatalog;
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
    services: ServiceCatalog<'trees>,
    calling_plans: &[BoundaryCallingPlanRealization],
    binding: &AcceptedSemanticBinding,
) -> Result<
    (
        &'trees checked_trees::typed_trees::trait_definition::TraitDefinition,
        effects::provider_plan::ServiceSchema,
    ),
    Diagnostic,
> {
    let typed = services.context().typed;
    let matches = services
        .declared_traits()
        .filter_map(|declaration| {
            if declaration.package != Some(binding.package())
                || declaration.path != binding.declaration_path()
            {
                return None;
            }
            let prepared = declaration.surface?;
            let definition = typed.trait_definition_by_symbol(declaration.schema.symbol())?;
            let schema = crate::service_schema::attach_calling(&prepared, calling_plans);
            (schema.declaration.trait_package_identity == Some(binding.package())
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
    services: ServiceCatalog<'_>,
    calling_plans: &[BoundaryCallingPlanRealization],
    accepted_binding: Option<&AcceptedSemanticBinding>,
    target_name: Option<&'static str>,
    already_selected: &[ProviderPlan],
    leaves: &[ToolchainSettledLeaf],
    provider: &'static str,
    host_label: &'static str,
) -> Result<Option<crate::DerivedProviderPlan>, Vec<Diagnostic>> {
    let Some(binding) = accepted_binding else {
        return Ok(None);
    };
    let Some(target_name) = target_name else {
        return Ok(None);
    };
    if leaves.is_empty() {
        return Ok(None);
    }
    let (definition, schema) = resolve_canonical_host_binding(services, calling_plans, binding)
        .map_err(|error| vec![error])?;
    let signatures = services
        .context()
        .typed
        .trait_machine_signatures(definition);
    if already_selected.iter().any(|selected| {
        selected.schema.declaration.trait_package_identity
            == schema.declaration.trait_package_identity
            && selected.schema.declaration.trait_name == schema.declaration.trait_name
    }) {
        return Ok(None);
    }
    let mut rows = Vec::with_capacity(leaves.len());
    let mut requirement_symbols = Vec::with_capacity(leaves.len());
    let mut identities = ProviderIdentityTable::default();
    for leaf in leaves {
        let Some(method) = schema
            .declaration
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
            admitted_same_stack_bytes: 0,
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
        name: crate::satisfies_plan_name(target_name, trait_name_text(&schema), provider),
        provider_type: provider.to_owned(),
        provider_type_package_identity: None,
        provider_application: Default::default(),
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
    let provenance = crate::ProviderPlanProvenance {
        schema: crate::ProviderSchemaDeclaration::BoundaryTrait(definition.symbol),
        provider_type: None,
        named_application: None,
        row_requirements: requirement_symbols,
        row_realizations: vec![crate::ProviderRowRealization::Toolchain; plan.rows.len()],
        row_target_machine_origins: vec![None; plan.rows.len()],
        row_accessors: vec![None; plan.rows.len()],
    };
    Ok(Some(crate::DerivedProviderPlan { plan, provenance }))
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
    pub settlement: ProviderSettlement<crate::DeferredProviderRows>,
    pub selected_provider_plan_facts: SelectedProviderPlanFacts,
    pub selected_provider_provenance: Vec<SelectedProviderReviewProvenance>,
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
    /// Complete execution declarations, including compiler-settled leaf plans.
    /// Callback projection and native handoff share this same closed roster.
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
    /// Every derived candidate plan plus toolchain-settled plans; trust
    /// replays each selected plan against exactly one of these.
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
    /// Consume captured declaration refusals after entry formation and before
    /// leaf completion, preserving the original error order without source.
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
    checked: &checked_trees::CheckedTrees,
    services: checked_trees::typed_trees::service_declarations::ServiceCatalog<'_>,
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
    prepared_providers: Result<crate::target_machines::PreparedProductProviders, Vec<Diagnostic>>,
    accepted_component_assumptions: &std::collections::BTreeSet<[u8; 32]>,
    boundary_calling_plan_realizations: Vec<BoundaryCallingPlanRealization>,
    calling_program: std::sync::Arc<crate::calling_policy_plans::CallingProgram>,
) -> Result<CheckedProviderSelection, Vec<Diagnostic>> {
    if !std::ptr::eq(services.context().typed, &*checked.typed)
        || services.context().scope != source::DependencyScope::Product
    {
        return Err(vec![Diagnostic::error(
            "provider settlement service catalog belongs to another Product program",
        )]);
    }
    let typed = &checked.typed;
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
        checked,
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
    let mut provider_declarations = canonical_selection.plans.clone();
    let selected_provider_plans = choices
        .into_iter()
        .map(
            |selected| crate::provider_planning::SelectedProviderPlanWithProvenance {
                derived: crate::DerivedProviderPlan {
                    plan: selected.candidate.plan,
                    provenance: selected.candidate.owner,
                },
                selected_by: crate::provider_planning::selection_declarations::restore_provenance(
                    selected.selected_by,
                ),
            },
        )
        .collect::<Vec<_>>();
    let direct_context = crate::provider_planning::external_binding_rows::ExternalBindingContext {
        calling_program: &calling_program,
        selected_target: target_name,
        native_target: provider_selection_target,
        boundary_calling_plan_realizations: &boundary_calling_plan_realizations,
    };
    let mut provider_candidates = product_candidates
        .into_iter()
        .map(|candidate| {
            candidate
                .rows
                .attach_physical_rows(candidate.source, &direct_context)
        })
        .collect::<Result<Vec<_>, _>>()?;
    // The canonical `FilesystemHost` slot admits no authored conformance or
    // selection row; the toolchain mints the per-target realization plan
    // itself. It joins extraction, fused-service erasure, closure facts, the
    // retained candidate inventory, and the post-derivation review provenance
    // below without entering the pre-selection derivation provenance.
    let selected_semantic_plans = selected_provider_plans
        .iter()
        .map(|selected| selected.derived.plan.clone())
        .collect::<Vec<_>>();
    let toolchain_filesystem_plan = canonical_filesystem_host::mint_canonical_filesystem_host_plan(
        services,
        &boundary_calling_plan_realizations,
        package_inputs.and_then(|inputs| {
            inputs.accepted_semantic_binding(AcceptedSemanticBindingRole::FilesystemHostService)
        }),
        target_name,
        &selected_semantic_plans,
    )?;
    // The cycle rule reads every closed checked instance, including uncalled ones,
    // and requires every schema method to bind a plan row. The toolchain plan covers
    // only its honestly realizable leaves and its syscall rows can never
    // invoke, so it joins after the authored-plan cycle check, before the
    // external-binding extraction that binds its syscall numbers.
    backend_plan::synchronous_invocations::validate_declared_synchronous_invocation_cycles(
        &provider_declarations,
        artifact.module(),
    )?;
    let external_binding_rows = crate::extract_native_external_binding_rows(
        target_name,
        provider_selection_target,
        &selected_provider_plans
            .iter()
            .map(|selected| &selected.derived.plan)
            .chain(
                toolchain_filesystem_plan
                    .iter()
                    .map(|derived| &derived.plan),
            )
            .collect::<Vec<_>>(),
        &boundary_calling_plan_realizations,
        &calling_program,
    )?;
    // Component-closure join. The verified components come only from the
    // descriptions attached to this compilation's package inputs, each
    // re-verified under the build's profile; every `Independent` selection
    // must be realized by exactly one of them and every component must
    // realize one selection, so a build that supplies none, several, an
    // unmatched extra, or a mismatched realization rejects here.
    let mut selected_provider_provenance = selected_provider_plans
        .into_iter()
        .map(|selected| SelectedProviderReviewProvenance {
            plan: selected.derived.plan,
            provider: selected.derived.provenance,
            selected_by: selected.selected_by,
            row_compiler_intrinsic_executions: Vec::new(),
            row_external_completion_leaves: Vec::new(),
        })
        .collect::<Vec<_>>();
    completion::retain_completions(
        checked,
        &mut selected_provider_provenance,
        target_name,
        &provider_candidates
            .iter()
            .filter_map(crate::ProviderPlanCandidate::derived)
            .collect::<Vec<_>>(),
    )?;
    if package_inputs.is_some_and(PackageCompilationInputs::semantic_role_discovery) {
        semantic_roles::validate_proposed(typed, &selected_provider_provenance)?;
    }
    for minted in [toolchain_filesystem_plan].into_iter().flatten() {
        selected_provider_plan_facts = selected_provider_plan_facts
            .with_toolchain_settled_plan(minted.plan.clone())
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        // Review provenance stays index-aligned with `facts.plans()` (the
        // intrinsic-review join zips them positionally); splice the toolchain
        // row at the same slot the sorted insert landed on. Its provenance
        // already records toolchain supply rather than an invented body.
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
                provider: minted.provenance,
                selected_by: crate::ProviderSelectionProvenance::UniqueCoveringCandidate,
                row_compiler_intrinsic_executions: Vec::new(),
                row_external_completion_leaves: vec![Vec::new(); minted.plan.rows.len()],
            },
        );
        provider_declarations.insert(
            index,
            backend_plan::TerminalTargetProviderPlan {
                plan: minted.plan.clone(),
                provider_declaration: None,
                row_numeric_realizations: vec![None; minted.plan.rows.len()],
            },
        );
        // The trust report replays every selected plan against exactly one
        // retained candidate; the toolchain-settled plan is a real retained
        // candidate for the slot even though no package authored it.
        provider_candidates.push(crate::ProviderPlanCandidate::toolchain(minted.plan));
    }
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
            provider_declarations: provider_declarations.into(),
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
    })
}
