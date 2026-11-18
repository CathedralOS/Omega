//! Rejoin the nominal entry selected by native planning with this exact executable publication.
use crate::SelectedProviderResult;
use diagnostics::Diagnostic;
use effects::semantic_bindings::{AcceptedSemanticBinding, AcceptedSemanticBindingRole};
use native_compiler::{PublishedEntryNomination, SelectedCompilerProgramEntry};
use semantic_vocabulary::compiler_evaluation::{DeclarationCustody, ProductDeclarationScope};
use std::sync::Arc;
use terminal_codec::CanonicalTerminalArtifact;

pub struct PublishedNativeEntry<'a> {
    artifact: &'a Arc<CanonicalTerminalArtifact>,
    nomination: &'a PublishedEntryNomination,
    providers: &'a [effects::provider_plan::ProviderPlan],
}

impl<'a> PublishedNativeEntry<'a> {
    pub fn for_program(
        artifact: &'a Arc<CanonicalTerminalArtifact>,
        execution: &'a SelectedProviderResult,
    ) -> Result<Self, Vec<Diagnostic>> {
        let refuse = |reason: String| vec![Diagnostic::error(reason)];
        let realization = execution.realization();
        realization
            .require_program(artifact.view().identity())
            .map_err(refuse)?;
        let nomination = execution.entry_nomination()?;
        if nomination.program() != execution.original_program()
            || nomination.source_signature().target_slot().owner
                != execution
                    .target_profile()
                    .unwrap_or_else(target::TargetProfile::host)
        {
            return Err(refuse(
                "entry nomination belongs to a different source program or deployment target"
                    .into(),
            ));
        }
        Ok(Self {
            artifact,
            nomination,
            providers: realization.provider_declarations(),
        })
    }

    pub fn source_signature(&self) -> &program_entry_plan::SelectedProgramEntrySignature {
        self.nomination.source_signature()
    }
    pub fn nominal_root(&self) -> Result<terminal_psi::PreparedNominalRoot, Vec<Diagnostic>> {
        self.nomination.nominal_root()
    }
    pub fn schema_declaration(&self) -> terminal_psi::TerminalIdentityIndex {
        self.nomination.schema_declaration()
    }
    pub fn binding_role(&self) -> Option<AcceptedSemanticBindingRole> {
        self.source_signature()
            .target_slot()
            .physical_contract_package
            .map(effects::semantic_bindings::program_entry_semantic_binding_role)
    }

    pub fn settle_calling_plans(
        self,
        requester: DeclarationCustody,
        binding: Option<&AcceptedSemanticBinding>,
        permissions: Option<&ProductDeclarationScope>,
    ) -> Result<SelectedCompilerProgramEntry, Vec<Diagnostic>> {
        native_compiler::plan_published_entry_calling_conventions(
            self.nomination,
            self.artifact,
            self.providers,
            requester,
            binding,
            permissions,
        )
    }
}
