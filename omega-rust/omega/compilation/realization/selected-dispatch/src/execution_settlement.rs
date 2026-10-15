//! Close execution dispatch, accepted host bindings, task activation and entry
//! establishment for one selected realization. Target/entry custody and x86
//! FMA associations leave with the program they were settled against. The
//! compiler neither projects service roles nor completes this result afterward.

use crate::provider_binding::CheckedProviderProgram;
use checked_trees::CheckedTrees as CheckedProgram;
use diagnostics::Diagnostic;
use package_compilation::AcceptedSemanticBindingRole;
use std::sync::Arc;

/// Checked semantics after selected execution has been settled in the exact
/// realization-owned dispatch order. This surface owns the now-closed review
/// provenance alongside every checked-phase sidecar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedExecution {
    pub program: Arc<CheckedProgram>,
    /// Target execution associations, never written into portable checked facts.
    pub dispatch: crate::selected_dispatch::SelectedExecutionDispatch,
    pub selected_target_profile: Option<target::TargetProfile>,
    pub selected_program_entry:
        Option<provider_planning::program_entry::SelectedCompilerProgramEntry>,
    /// Exact admitted deployment provider; absence retains the SSE2 baseline.
    pub x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
    /// Exact source demands joined to that provider, never fabricated by admission.
    pub x86_scalar_fma_plan_associations:
        Vec<provider_planning::x86_fma_plan_association::CheckedX86ScalarFmaPlanAssociation>,
    pub selected_provider_plan_facts: effects::SelectedProviderPlanFacts,
    /// Authored Build grants resolved to the exact selected plans.
    pub selected_provider_grants: Vec<trust_model::ResolvedAuthoredSelectedProviderGrant>,
    /// Target-owned recipes joined to checked nominal uses. Engines consume
    /// these plans rather than reconstructing placement from source semantics.
    pub callback_placements:
        Vec<provider_planning::calling_policy_plans::SelectedCallbackPlacement>,
    pub accepted_template_classifications: trust_model::AcceptedTemplateClassifications,
    pub selected_provider_provenance: Vec<provider_planning::SelectedProviderReviewProvenance>,
    pub resolved_semantic_bindings: Vec<crate::ResolvedAcceptedSemanticBinding>,
    pub component_progress: Option<effects::ComponentProgressManifest>,
    pub task_activations: task_plans::TaskActivationPlanSet,
}

impl SelectedExecution {
    /// Native ABI of the selected deployment profile. Profiles that share an
    /// ABI, notably `windows_x64` and `uefi_x64`, stay distinct in
    /// `selected_target_profile`. Semantic-only checking selects neither and
    /// therefore cannot be staged.
    pub fn selected_native_target(&self) -> Option<target::NativeTarget> {
        self.selected_target_profile
            .map(target::TargetProfile::native_target)
    }

    /// Boundaries demanded by the selected ProgramEntry: its receiver's
    /// `Binding<R>` fields, including those nested in record fields, then the
    /// boundaries its own service reach names. A ProgramEntry is root-bound
    /// rather than a package callable, so package review never reads its
    /// reach as a callable's: this is the consumer's own nomination surface.
    /// Establishment reads the same field traversal, so every field nominated
    /// here is a field establishment later requires a selected Fused provider
    /// for.
    pub fn program_entry_service_requirements(&self) -> Vec<symbols::SymbolHandle> {
        let Some(entry) = self.selected_program_entry.as_ref() else {
            return Vec::new();
        };
        crate::program_entry_service_requirements(&self.program, entry.machine_symbol())
    }

    /// Whether the selected program entry tolerated a `Binding` field whose
    /// Fused provider was never selected. Only a discovery pass may carry such
    /// a result: a strict recompile must reject the same program, so a result
    /// answering `true` is not interchangeable with one produced without the
    /// tolerance.
    pub fn tolerated_unsettled_service_fields(&self) -> bool {
        self.selected_program_entry.as_ref().is_some_and(|entry| {
            crate::service_custody::program_entry_has_unsettled_service_fields(
                &self.program,
                entry.machine_symbol(),
                &self.selected_provider_provenance,
            )
        })
    }

    pub fn resolved_semantic_binding(
        &self,
        role: AcceptedSemanticBindingRole,
    ) -> Option<&crate::ResolvedAcceptedSemanticBinding> {
        self.resolved_semantic_bindings
            .iter()
            .find(|binding| binding.role() == role)
    }

    /// Exact consumer-policy semantic bindings that were resolved and consumed
    /// by this checked package compilation. Unconsumed or stale input bindings
    /// reject before selected execution settles.
    pub fn accepted_semantic_bindings(
        &self,
    ) -> impl ExactSizeIterator<Item = &package_compilation::AcceptedSemanticBinding> {
        self.resolved_semantic_bindings
            .iter()
            .map(crate::ResolvedAcceptedSemanticBinding::accepted)
    }

    /// Permission rows from the exact semantic bindings this execution
    /// consumed. Proposal construction and accepted-policy admission project
    /// from the checked source independently, never from each other.
    pub fn terminal_authority_permissions(
        &self,
    ) -> (
        Vec<effects::ServiceTerminalAuthorityPermission>,
        terminal_psi::TerminalIdentityTable,
    ) {
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let mut permissions = Vec::new();
        for binding in self.accepted_semantic_bindings() {
            for permission in binding.terminal_authority_permissions() {
                let mut permission = permission.clone();
                permission.rekey_requirement_identity(
                    identities.intern(
                        binding
                            .terminal_authority_permission_identities()
                            .text(permission.requirement_identity())
                            .unwrap_or_default(),
                    ),
                );
                permissions.push(permission);
            }
        }
        (permissions, identities.build())
    }

    /// Construct a non-authoritative review candidate for one exact package-
    /// owned requirement-only boundary. Readable role nomination happens
    /// outside the compiler; this contributes only checked owner, nominal, and
    /// normalized-schema coordinates for a required bound replay.
    pub fn candidate_service_binding(
        &self,
        calling_plans: &[provider_planning::calling_policy_plans::BoundaryCallingPlanRealization],
        role: AcceptedSemanticBindingRole,
        package: semantic_vocabulary::PackageKeyIdentity,
        declaration_path: &str,
    ) -> Result<package_compilation::AcceptedSemanticBinding, Diagnostic> {
        let typed = &self.program.typed;
        let matches = typed
            .traits()
            .iter()
            .filter(|definition| {
                definition.is_boundary
                    && typed.symbols.symbol_package_identity(definition.symbol) == Some(package)
                    && typed.symbols.display_path(definition.symbol, "::") == declaration_path
            })
            .filter_map(|definition| {
                provider_planning::service_schema::from_typed(
                    provider_planning::ProviderPlanningContext::new(
                        typed,
                        source::DependencyScope::Product,
                    ),
                    calling_plans,
                    definition,
                )
            })
            .collect::<Vec<_>>();
        let [schema] = matches.as_slice() else {
            return Err(Diagnostic::error(format!(
                "semantic-binding candidate {:?} resolved to {} exact package-owned boundary schemas instead of one",
                role,
                matches.len(),
            )));
        };
        package_compilation::AcceptedSemanticBinding::new_service(
            role,
            package,
            declaration_path,
            package_compilation::accepted_service_schema_digest(role, schema),
        )
        .map_err(Diagnostic::error)
    }
}

/// Whole owner inputs, not independently projected service bindings or a
/// second asserted component root. This remains a checked-tree realization
/// boundary; it does not claim the intended Terminal-only provider handoff.
pub struct SelectedExecutionRequest<'a> {
    /// Provider derivation target, distinct from executable deployment selection.
    pub provider_planning_target_profile: Option<target::TargetProfile>,
    pub calling_plans:
        &'a [provider_planning::calling_policy_plans::BoundaryCallingPlanRealization],
    pub selected_program_entry:
        Option<provider_planning::program_entry::SelectedCompilerProgramEntry>,
    pub selected_target_profile: Option<target::TargetProfile>,
    pub package_inputs: Option<&'a package_compilation::PackageCompilationInputs>,
    pub x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
    pub permit_unsettled_fused_service_fields: bool,
    pub selected_provider_provenance: Vec<provider_planning::SelectedProviderReviewProvenance>,
    pub opaque_representation_selections:
        &'a [representation_planning::OpaqueRepresentationSelection],
}

/// Consume a complete checked surface and settle every selected execution
/// rewrite before publishing the final compiler-facing surface.
pub fn settle_selected_execution(
    checked: CheckedProviderProgram,
    mut settlement: SelectedExecutionRequest<'_>,
) -> Result<SelectedExecution, Vec<Diagnostic>> {
    if settlement.selected_target_profile.is_some()
        && settlement.selected_target_profile != settlement.provider_planning_target_profile
    {
        return Err(vec![Diagnostic::error(
            "selected deployment target differs from retained provider planning target",
        )]);
    }
    // The same selected entry supplies component progress and receives fused
    // field establishments below. Callers cannot nominate an unrelated root
    // beside it or pair a native ABI with a different deployment profile.
    let exact_component_progress_root = settlement.selected_program_entry.as_ref().map(|entry| {
        let source = entry.source_signature();
        provider_planning::component_progress::ExactComponentProgressRoot::new(
            entry.machine_symbol(),
            source
                .identities()
                .text(source.normalized_callable_identity())
                .unwrap_or_default(),
        )
    });
    let component_progress =
        provider_planning::component_progress::build_selected_component_progress_manifest(
            &checked.program,
            &checked.program.facts,
            &checked.selected_provider_plan_facts,
            exact_component_progress_root,
            None,
        )?;
    let dispatch = crate::settle_selected_execution_dispatch(
        &checked.program,
        settlement.calling_plans,
        &checked.selected_provider_plan_facts,
        &settlement.selected_provider_provenance,
    )?;
    let accepted_binding = |role| {
        settlement
            .package_inputs
            .and_then(|inputs| inputs.accepted_semantic_binding(role))
    };
    let resolve_service_binding =
        |binding: Option<&package_compilation::AcceptedSemanticBinding>| {
            binding
                .map(|binding| {
                    crate::resolve_accepted_service_binding(
                        &checked.program,
                        settlement.calling_plans,
                        binding,
                    )
                })
                .transpose()
                .map_err(|diagnostic| vec![diagnostic])
        };
    let resolved_exit_bindings = crate::retain_selected_compiler_intrinsic_review_identities(
        &checked.program,
        settlement.calling_plans,
        &checked.selected_provider_plan_facts,
        &mut settlement.selected_provider_provenance,
        settlement
            .provider_planning_target_profile
            .map(target::TargetProfile::target_name),
        accepted_binding(AcceptedSemanticBindingRole::ConsoleExitProcessI32),
        accepted_binding(AcceptedSemanticBindingRole::ProcessExitExitProcessI32),
    )?;
    let resolved_filesystem_binding = resolve_service_binding(accepted_binding(
        AcceptedSemanticBindingRole::FilesystemHostService,
    ))?;
    let resolved_time_host_binding = resolve_service_binding(accepted_binding(
        AcceptedSemanticBindingRole::TimeHostService,
    ))?;
    // Package lookup is keyed by the exact role of this target's entry slot;
    // the coordinator no longer supplies a separately asserted expected role.
    let resolved_entry_binding = resolve_service_binding(
        settlement
            .selected_target_profile
            .map(|profile| profile.program_entry_slot())
            .and_then(|slot| slot.physical_contract_package)
            .map(package_compilation::program_entry_semantic_binding_role)
            .and_then(accepted_binding),
    )?;
    let task_activations = provider_planning::task_plans::elaborate_task_activation_plans(
        &checked.program,
        &checked.program.facts,
        &checked.selected_provider_plan_facts,
        settlement
            .selected_target_profile
            .map(target::TargetProfile::native_target)
            .unwrap_or_else(target::NativeTarget::host),
        settlement.opaque_representation_selections,
    )?;

    let x86_scalar_fma_plan_associations =
        provider_planning::x86_fma_plan_association::bind_checked_x86_scalar_fma_plan_associations(
            &checked.program,
            &checked.program.facts,
            &checked.selected_provider_plan_facts,
            &settlement.selected_provider_provenance,
            settlement.x86_scalar_fma_provider,
            settlement.selected_target_profile,
        )?;
    if let Some(entry) = settlement.selected_program_entry.as_mut() {
        // Establishments intern into the source signature's own identity
        // table so every ProgramEntry identity shares one ordering.
        let mut identities = entry.source_signature().identities().clone().into_builder();
        let establishments = crate::derive_fused_program_entry_establishments(
            &checked.program,
            settlement.calling_plans,
            entry,
            &settlement.selected_provider_provenance,
            settlement.permit_unsettled_fused_service_fields,
            &mut identities,
        )?;
        entry.set_source_identities(identities.build());
        entry
            .bind_fused_service_establishments(establishments)
            .map_err(|message| vec![Diagnostic::error(message)])?;
    }

    Ok(SelectedExecution {
        program: checked.program,
        dispatch,
        selected_target_profile: settlement.selected_target_profile,
        selected_program_entry: settlement.selected_program_entry,
        x86_scalar_fma_provider: settlement.x86_scalar_fma_provider,
        x86_scalar_fma_plan_associations,
        selected_provider_plan_facts: checked.selected_provider_plan_facts,
        selected_provider_grants: checked.selected_provider_grants,
        callback_placements: checked.callback_placements,
        accepted_template_classifications: checked.accepted_template_classifications,
        selected_provider_provenance: settlement.selected_provider_provenance,
        resolved_semantic_bindings: resolved_exit_bindings
            .into_iter()
            .chain(resolved_filesystem_binding)
            .chain(resolved_time_host_binding)
            .chain(resolved_entry_binding)
            .collect(),
        component_progress,
        task_activations,
    })
}
