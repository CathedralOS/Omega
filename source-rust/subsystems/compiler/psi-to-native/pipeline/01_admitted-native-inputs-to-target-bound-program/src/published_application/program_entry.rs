//! Rejoin the nominal entry selected by native planning with this exact executable publication.
use crate::program_entry::{
    PublishedEntryNomination, SelectedCompilerProgramEntry,
    plan_published_entry_calling_conventions,
};
use crate::{PublishedSelection, SelectedExecution};
use declaration_scope::ProductDeclarationScope;
use diagnostics::Diagnostic;
use semantic_vocabulary::compiler_evaluation::DeclarationCustody;
use std::sync::Arc;
use terminal_codec::CanonicalTerminalArtifact;

pub(super) struct PublishedNativeEntry<'a> {
    artifact: &'a Arc<CanonicalTerminalArtifact>,
    nomination: &'a PublishedEntryNomination,
    providers: &'a [provider_plans::ProviderPlan],
}

impl<'a> PublishedNativeEntry<'a> {
    pub fn for_program<Publisher>(
        artifact: &'a Arc<CanonicalTerminalArtifact>,
        execution: &'a PublishedSelection<Publisher, SelectedExecution>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let refuse = |reason: String| vec![Diagnostic::error(reason)];
        let realization = execution.realization();
        realization
            .require_program(artifact.view().identity())
            .map_err(refuse)?;
        let nomination = execution.entry_nomination()?;
        if nomination.program() != execution.original_program()
            || Some(nomination.source_signature().target_slot().owner) != execution.target_profile()
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

    pub fn schema_declaration(&self) -> terminal_psi::TerminalIdentityIndex {
        self.nomination.schema_declaration()
    }
    pub fn settle_calling_plans(
        self,
        requester: DeclarationCustody,
        permissions: Option<&ProductDeclarationScope>,
    ) -> Result<SelectedCompilerProgramEntry, Vec<Diagnostic>> {
        plan_published_entry_calling_conventions(
            self.nomination,
            self.artifact,
            self.providers,
            requester,
            permissions,
        )
    }
}
