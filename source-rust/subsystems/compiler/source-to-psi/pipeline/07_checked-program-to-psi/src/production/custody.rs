//! Source-issued Build operations travel with the checked code.
//! Portable publication seals the declaration owners and evidence into the artifact.

use crate::{TerminalBuildBindings, TerminalBuildFilesystem, TerminalBuildRequests};
use psi_authorization::selection_questions::ProductSchemaCustody;
use std::sync::Arc;
use terminal_codec::{CanonicalTerminalArtifact, CanonicalTerminalArtifactError};

#[derive(Debug, PartialEq, Eq)]
#[must_use = "Terminal production retains the published code and its Build companions"]
pub struct ProducedTerminalProgram {
    pub(crate) program: terminal_codec::TerminalProgramProduct,
    pub(crate) evidence: Option<Arc<terminal_codec::EvidenceSeals>>,
    pub(crate) build_bindings: TerminalBuildBindings,
    pub(crate) build_requests: TerminalBuildRequests,
    pub(crate) build_filesystem: TerminalBuildFilesystem,
    pub(crate) provenance: psi_publication_records::TerminalMachineProvenance,
    pub(crate) source_records: Vec<crate::source_state::SourceRecordCorrespondence>,
    pub(crate) source_selections: Option<Arc<crate::TerminalSourceSelections>>,
}

impl ProducedTerminalProgram {
    pub const fn evidence(&self) -> Option<&Arc<terminal_codec::EvidenceSeals>> {
        self.evidence.as_ref()
    }
    /// Compiler invocations need the source type applications produced beside
    /// their checked code. Borrow those existing rows from the same code owner.
    pub fn source_records(&self) -> &[crate::source_state::SourceRecordCorrespondence] {
        &self.source_records
    }
    pub fn source_selections(&self) -> Option<&Arc<crate::TerminalSourceSelections>> {
        self.source_selections.as_ref()
    }
    /// Seal source publication metadata into the artifact.
    pub fn into_portable_publication(
        self,
    ) -> Result<CanonicalTerminalArtifact, CanonicalTerminalArtifactError> {
        let artifact = self.program.into_portable()?;
        let view = artifact.view().into();
        let owners = self.source_selections.as_deref().map(|selections| {
            selections
                .questions_for(view)
                .map(|questions| ProductSchemaCustody::from_selections(view, questions))
                .ok_or_else(|| {
                    "source declaration custody belongs to another program or invocation".to_owned()
                })
        });
        let entry_metadata =
            crate::source::entry::issue(&artifact, owners.as_ref(), self.provenance);
        artifact.with_publication(Arc::new(entry_metadata), self.evidence)
    }
    pub const fn build_filesystem(&self) -> &TerminalBuildFilesystem {
        &self.build_filesystem
    }
    pub const fn build_requests(&self) -> &TerminalBuildRequests {
        &self.build_requests
    }
    pub const fn build_bindings(&self) -> &TerminalBuildBindings {
        &self.build_bindings
    }
    pub fn view(&self) -> terminal_codec::TerminalExecutionView<'_> {
        self.program.view()
    }

    pub fn retain_code(&self) -> terminal_codec::RetainedTerminalCode {
        self.program.retain_code()
    }

    pub fn artifact(&self) -> Result<&CanonicalTerminalArtifact, CanonicalTerminalArtifactError> {
        self.program.portable()
    }
}

#[derive(Clone, Copy, Default)]
pub struct TerminalProductionCustody<'a> {
    /// Captured anonymous Build source may be used only in this invocation.
    pub invocation_sources: Option<&'a dyn crate::source_inputs::InvocationSourceConsent>,
}
