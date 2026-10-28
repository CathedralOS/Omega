//! Complete source-prepared execution for one selected realization.
//!
//! Provider binding produces this immutable result. Terminal production borrows
//! it; native admission still joins actual demand and receiving permissions.

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
    pub service_declarations: Arc<provider_planning::service_declarations::ServiceDeclarations>,
    pub external_providers: Arc<psi_terminal_production::PreparedExternalProviderDeclarations>,
    /// Only native handoff demands these source identities. A refusal remains
    /// data so checking-only stops acquire no native-only obligations.
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
        role: AcceptedSemanticBindingRole,
        package: semantic_vocabulary::PackageKeyIdentity,
        declaration_path: &str,
    ) -> Result<package_compilation::AcceptedSemanticBinding, Diagnostic> {
        let service = self
            .service_declarations
            .select(package, declaration_path)?;
        let schema = &service.declaration.schema;
        package_compilation::AcceptedSemanticBinding::new_service(
            role,
            package,
            declaration_path,
            package_compilation::accepted_service_schema_digest(role, schema),
        )
        .map_err(Diagnostic::error)
    }
}
