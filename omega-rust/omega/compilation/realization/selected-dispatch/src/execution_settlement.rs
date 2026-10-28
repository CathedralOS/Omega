//! Close accepted host bindings and entry
//! establishment for one selected realization. Target/entry custody leaves with
//! the selected program; native provider admission checks optional ISA features
//! against actual Terminal demand. The compiler neither projects service roles
//! nor completes this result afterward.

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
    /// Complete direct-call supplies, constructed after host bindings resolve.
    /// Native demand and receiving authority still decide which may execute.
    pub direct_boundary_leaves: Vec<backend_plan::DirectBoundaryLeaf>,
    pub program: Arc<CheckedProgram>,
    pub external_providers: Arc<psi_terminal_production::ExternalProviderDeclarations>,
    pub native_provider_sources: Result<Arc<backend_plan::NativeProviderSources>, Vec<Diagnostic>>,
    pub selected_target_profile: Option<target::TargetProfile>,
    pub selected_program_entry:
        Option<provider_planning::program_entry::SelectedCompilerProgramEntry>,
    /// Exact admitted deployment provider; absence retains the SSE2 baseline.
    pub x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
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

    /// Boundaries in the selected entry's checked service reach. Entry selection
    /// nominates exact provider requirements; it constructs no receiver or fields.
    /// A ProgramEntry is root-bound rather than a package callable, so this is
    /// the consumer's own nomination surface, not a package callable projection.
    pub fn program_entry_service_requirements(&self) -> Vec<symbols::SymbolHandle> {
        let Some(entry) = self.selected_program_entry.as_ref() else {
            return Vec::new();
        };
        crate::program_entry_service_requirements(&self.program, entry.machine_symbol())
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
    pub provider_candidates: &'a [provider_planning::ProviderPlanCandidate],
    /// Provider derivation target, distinct from executable deployment selection.
    pub provider_planning_target_profile: Option<target::TargetProfile>,
    pub calling_plans:
        &'a [provider_planning::calling_policy_plans::BoundaryCallingPlanRealization],
    pub selected_program_entry:
        Option<provider_planning::program_entry::SelectedCompilerProgramEntry>,
    pub selected_target_profile: Option<target::TargetProfile>,
    pub package_inputs: Option<&'a package_compilation::PackageCompilationInputs>,
    pub x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
    pub selected_provider_provenance: Vec<provider_planning::SelectedProviderReviewProvenance>,
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
        &checked.external_providers,
        settlement.calling_plans,
        &checked.selected_provider_plan_facts,
        &mut settlement.selected_provider_provenance,
        settlement
            .provider_planning_target_profile
            .map(target::TargetProfile::target_name),
        accepted_binding(AcceptedSemanticBindingRole::ConsoleExitProcessI32),
        accepted_binding(AcceptedSemanticBindingRole::ProcessExitExitProcessI32),
    )?;
    let direct_boundary_leaves = crate::intrinsic_review::produce_direct_boundary_leaves(
        &checked.program,
        &checked.external_providers,
        &settlement.selected_provider_provenance,
        settlement.provider_candidates,
        settlement
            .provider_planning_target_profile
            .map(target::TargetProfile::target_name),
        &resolved_exit_bindings,
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
    Ok(SelectedExecution {
        direct_boundary_leaves,
        external_providers: checked.external_providers,
        native_provider_sources: checked.native_provider_sources,
        program: checked.program,
        selected_target_profile: settlement.selected_target_profile,
        selected_program_entry: settlement.selected_program_entry,
        x86_scalar_fma_provider: settlement.x86_scalar_fma_provider,
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
    })
}
