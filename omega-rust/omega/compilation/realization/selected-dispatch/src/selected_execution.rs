// Target execution uses saved declarations and selected plans without access
// to checked source; explicit source inspection and the checked interpreter
// use a separate owner on CheckedCompilation outside this record.
//
// SelectedExecution holds the complete selected implementation data for one
// target:
// - selected_native_target returns the native ABI of the selected deployment
//   profile while keeping profiles that share an ABI (such as windows_x64 and
//   uefi_x64) distinct in selected_target_profile; semantic-only checking
//   selects neither and cannot be staged.
// - program_entry_service_requirements returns boundaries in the selected
//   entry's checked service reach, nominating exact provider requirements
//   without constructing a receiver or fields.
// - accepted_semantic_bindings and terminal_authority_permissions expose
//   consumer-policy semantic bindings and permission rows resolved and consumed
//   by this checked package compilation, while candidate_service_binding
//   constructs a non-authoritative review candidate for one exact package-owned
//   requirement-only boundary.

use diagnostics::Diagnostic;
use package_compilation::AcceptedSemanticBindingRole;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedExecution {
    pub direct_boundary_leaves: Vec<backend_plan::DirectBoundaryLeaf>, // complete direct-call supplies constructed after host bindings resolve
    pub selected_provider_plan_facts: Arc<effects::SelectedProviderPlanFacts>,
    pub entry_service_requirements: Vec<symbols::SymbolHandle>,
    pub trust_declarations: Arc<language_semantics::trust_declarations::TrustDeclarations>,
    pub has_proof_obligations: bool,
    pub service_declarations: Arc<provider_planning::service_declarations::ServiceDeclarations>,
    pub external_providers: Arc<provider_planning::PreparedExternalProviderDeclarations>,
    pub native_provider_sources: Result<Arc<backend_plan::NativeProviderSources>, Vec<Diagnostic>>, // refusal remains data so checking-only stops acquire no native-only obligations
    pub selected_target_profile: Option<target::TargetProfile>,
    pub selected_program_entry:
        Option<provider_planning::program_entry::SelectedCompilerProgramEntry>,
    pub x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>, // exact admitted deployment provider; absence retains the SSE2 baseline
    pub selected_provider_grants: Vec<trust_model::ResolvedAuthoredSelectedProviderGrant>, // authored Build grants resolved to exact selected plans
    pub callback_roots: terminal_psi::PreparedTerminalRoots, // selected canonical declarations for behavior admission, including dormant uses
    pub selected_provider_provenance: Vec<provider_planning::SelectedProviderReviewProvenance>,
    pub resolved_semantic_bindings: Vec<crate::ResolvedAcceptedSemanticBinding>,
}

impl SelectedExecution {
    pub fn selected_native_target(&self) -> Option<target::NativeTarget> {
        self.selected_target_profile
            .map(target::TargetProfile::native_target)
    }

    pub fn program_entry_service_requirements(&self) -> Vec<symbols::SymbolHandle> {
        self.entry_service_requirements.clone()
    }

    pub fn plans(&self) -> &effects::SelectedProviderPlanFacts {
        &self.selected_provider_plan_facts
    }

    pub fn resolved_semantic_binding(
        &self,
        role: AcceptedSemanticBindingRole,
    ) -> Option<&crate::ResolvedAcceptedSemanticBinding> {
        self.resolved_semantic_bindings
            .iter()
            .find(|binding| binding.role() == role)
    }

    pub fn accepted_semantic_bindings(
        &self,
    ) -> impl ExactSizeIterator<Item = &package_compilation::AcceptedSemanticBinding> {
        self.resolved_semantic_bindings
            .iter()
            .map(crate::ResolvedAcceptedSemanticBinding::accepted)
    }

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
