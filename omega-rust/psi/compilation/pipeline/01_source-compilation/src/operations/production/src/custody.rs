/*
    Retained Terminal production outputs (`ProducedTerminalProgram` and
    `PortableTerminalProgram`) and independent application judgment custody.

    We pair canonical or invocation-scoped Terminal code with its source-issued
    Build companions, machine provenance, and per-application root and
    `ProgramEntry` receipts, supporting compiler-storage record persistence and
    late application attachment without reopening source trees.
*/

use crate::declaration_selection::NativeBindingFormation;
use crate::declaration_selection::ProductDeclarationRequest;
use crate::production::ProducedTerminalApplication;
use crate::production::TerminalBuildFilesystemCall;
use crate::production::{TerminalBuildBindings, TerminalBuildFilesystem, TerminalBuildRequests};
use compiler_storage::RecordWriter;
use diagnostics::Diagnostic;
use std::sync::Arc;
use terminal_codec::entry_receipt::rejoin_program_entry_receipt;
use terminal_codec::{CanonicalTerminalArtifact, CanonicalTerminalArtifactError};

// Operation numbers are local to their machine. Refuse persisted records that
// predate the complete call-site keys before decoding their shifted fields.
const PRODUCED_PROGRAM_SCHEMA: u64 = u64::from_le_bytes(*b"OMGBLD02");

#[cfg(test)]
mod schema_tests {
    #[test]
    fn old_companion_rows_refuse_before_artifact_decoding() {
        let old_first_field = b"PSITERM\0".to_vec();
        let bytes = compiler_storage::encode(&old_first_field, Default::default()).unwrap();
        let error =
            compiler_storage::decode::<super::ProducedTerminalProgram>(&bytes, Default::default())
                .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("unsupported produced program record schema"),
            "{error}"
        );
    }
}

// An emitted call site does not authenticate its saved source action or Product
// operands. These records need their accepted source correspondence rejoined.
fn require_saved_build_correspondence(
    bindings: &[crate::production::TerminalBuildBindingCall],
    declarations: &[Arc<ProductDeclarationRequest>],
    queries: &[crate::production::TerminalBuildQueryCall],
    native_formations: &[NativeBindingFormation],
    requests: &[crate::production::TerminalBuildRequestCall],
    filesystem: &[TerminalBuildFilesystemCall],
) -> Result<(), &'static str> {
    if bindings.is_empty()
        && declarations.is_empty()
        && queries.is_empty()
        && native_formations.is_empty()
        && requests.is_empty()
        && filesystem.is_empty()
    {
        Ok(())
    } else {
        Err("saved Build companions require their accepted source protocol correspondence")
    }
}

#[derive(Debug, PartialEq, Eq)]
#[must_use = "Terminal production retains independent application judgments"]
pub struct ProducedTerminalProgram {
    pub(crate) program: terminal_codec::TerminalProgramProduct,
    pub(crate) evidence: Option<Arc<terminal_codec::EvidenceSeals>>,
    pub(crate) build_bindings: TerminalBuildBindings,
    pub(crate) build_requests: TerminalBuildRequests,
    pub(crate) build_filesystem: TerminalBuildFilesystem,
    pub(crate) applications: Vec<Result<ProducedTerminalApplication, Vec<Diagnostic>>>,
    pub(crate) provenance: terminal_psi::TerminalMachineProvenance,
    pub(crate) source_records: Vec<crate::source_state::SourceRecordCorrespondence>,
    pub(crate) source_selections: Option<Arc<crate::production::TerminalSourceSelections>>,
}

impl compiler_storage::Record for ProducedTerminalProgram {
    fn write(&self, output: &mut RecordWriter) -> Result<(), compiler_storage::RecordError> {
        output.record(|output| {
            let view = self.view();
            macro_rules! companion {
                ($value:expr) => {
                    $value
                        .ok_or_else(|| output.error("Terminal companion lost its artifact owner"))?
                };
            }
            let calls = companion!(self.build_bindings.calls_for(view));
            let declarations = companion!(self.build_bindings.declarations_for(view));
            let queries = companion!(self.build_bindings.queries_for(view));
            let native_formations = companion!(self.build_bindings.native_formations_for(view));
            let requests = companion!(self.build_requests.calls_for(view));
            let filesystem = companion!(self.build_filesystem.calls_for(view));
            require_saved_build_correspondence(
                calls,
                declarations,
                queries,
                native_formations,
                requests,
                filesystem,
            )
            .map_err(|reason| output.error(reason))?;
            PRODUCED_PROGRAM_SCHEMA.write(output)?;
            self.artifact()
                .map_err(|error| output.error(error.to_string()))?
                .to_bytes()
                .write(output)?;
            calls.write(output)?;
            declarations.write(output)?;
            queries.write(output)?;
            native_formations.write(output)?;
            requests.write(output)?;
            filesystem.write(output)?;
            self.applications.write(output)?;
            self.provenance.write(output)?;
            self.source_records.write(output)
        })
    }

    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        use compiler_storage::Record;
        input.record(|input| {
            if u64::read(input)? != PRODUCED_PROGRAM_SCHEMA {
                return Err(input.error("unsupported produced program record schema"));
            }
            let bytes = Vec::<u8>::read(input)?;
            let artifact = CanonicalTerminalArtifact::from_bytes(&bytes)
                .map_err(|error| input.error(error.to_string()))?;
            let calls: Arc<[crate::production::TerminalBuildBindingCall]> = Record::read(input)?;
            let declarations: Arc<[Arc<ProductDeclarationRequest>]> = Record::read(input)?;
            let queries: Arc<[crate::production::TerminalBuildQueryCall]> = Record::read(input)?;
            let native_formations: Arc<[NativeBindingFormation]> = Record::read(input)?;
            let requests: Arc<[crate::production::TerminalBuildRequestCall]> = Record::read(input)?;
            let filesystem: Arc<[TerminalBuildFilesystemCall]> = Record::read(input)?;
            // Code identity authenticates the emitted call, not the independently
            // decoded protocol action or Product operands. Only accepted source
            // currently establishes that correspondence. Portable Product code
            // with no Build companions remains loadable; nonempty Build records
            // need that source-issued provenance rejoined before host admission.
            require_saved_build_correspondence(
                &calls,
                &declarations,
                &queries,
                &native_formations,
                &requests,
                &filesystem,
            )
            .map_err(|reason| input.error(reason))?;
            let applications: Vec<Result<ProducedTerminalApplication, Vec<Diagnostic>>> =
                Record::read(input)?;
            let provenance = Record::read(input)?;
            let source_records = Record::read(input)?;
            for application in applications.iter().flatten() {
                if let Ok(Some(receipt)) = &application.receipt {
                    rejoin_program_entry_receipt(receipt, &artifact)
                        .map_err(|error| input.error(error.to_string()))?;
                }
                for root in application
                    .roots
                    .iter()
                    .filter_map(|root| root.as_ref().ok())
                {
                    if artifact.semantic_module().machine(root.machine).is_none() {
                        return Err(input.error("composition root lost its canonical machine"));
                    }
                }
            }
            let view = artifact.view().into();
            crate::production::build_bindings::validate_companions(
                view,
                &calls,
                &queries,
                &requests,
                &filesystem,
            )
            .map_err(|reason| input.error(reason))?;
            let build_bindings =
                TerminalBuildBindings::new(view, calls, declarations, queries, native_formations);
            let build_requests = TerminalBuildRequests::new(view, requests);
            let build_filesystem = TerminalBuildFilesystem::new(view, filesystem);
            Ok(Self {
                program: terminal_codec::TerminalProgramProduct::Portable(artifact),
                evidence: None,
                build_bindings,
                build_requests,
                build_filesystem,
                applications,
                provenance,
                source_records,
                // Persisted code is not a new source-issued permission packet.
                source_selections: None,
            })
        })
    }
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
    pub fn source_selections(&self) -> Option<&Arc<crate::production::TerminalSourceSelections>> {
        self.source_selections.as_ref()
    }
    pub fn into_shared_portable(
        self,
    ) -> Result<PortableTerminalProgram, CanonicalTerminalArtifactError> {
        let artifact = self.program.into_portable()?;
        let entry_metadata = super::published_entry::issue(
            &artifact,
            self.source_selections.as_deref(),
            self.provenance,
        );
        Ok(PortableTerminalProgram {
            artifact: Arc::new(artifact),
            evidence: self.evidence,
            build_bindings: self.build_bindings,
            build_requests: self.build_requests,
            build_filesystem: self.build_filesystem,
            applications: self.applications,
            entry_metadata: Arc::new(entry_metadata),
            source_records: self.source_records,
            source_selections: self.source_selections,
        })
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

    pub fn into_artifact(
        self,
    ) -> Result<CanonicalTerminalArtifact, CanonicalTerminalArtifactError> {
        self.program.into_portable()
    }

    pub fn into_parts(
        self,
    ) -> Result<
        (
            CanonicalTerminalArtifact,
            Vec<Result<ProducedTerminalApplication, Vec<Diagnostic>>>,
        ),
        CanonicalTerminalArtifactError,
    > {
        Ok((self.program.into_portable()?, self.applications))
    }
}

#[derive(Debug)]
pub struct PortableTerminalProgram {
    artifact: Arc<CanonicalTerminalArtifact>,
    evidence: Option<Arc<terminal_codec::EvidenceSeals>>,
    build_bindings: TerminalBuildBindings,
    build_requests: TerminalBuildRequests,
    build_filesystem: TerminalBuildFilesystem,
    applications: Vec<Result<ProducedTerminalApplication, Vec<Diagnostic>>>,
    entry_metadata: Arc<terminal_psi::PublishedEntryMetadata>,
    source_records: Vec<crate::source_state::SourceRecordCorrespondence>,
    source_selections: Option<Arc<crate::production::TerminalSourceSelections>>,
}

impl PortableTerminalProgram {
    pub fn entry_metadata(&self) -> &Arc<terminal_psi::PublishedEntryMetadata> {
        &self.entry_metadata
    }

    pub fn machine_provenance(&self) -> &terminal_psi::TerminalMachineProvenance {
        self.entry_metadata.machine_provenance()
    }

    pub const fn evidence(&self) -> Option<&Arc<terminal_codec::EvidenceSeals>> {
        self.evidence.as_ref()
    }
    pub fn source_records(&self) -> &[crate::source_state::SourceRecordCorrespondence] {
        &self.source_records
    }
    pub fn source_selections(&self) -> Option<&Arc<crate::production::TerminalSourceSelections>> {
        self.source_selections.as_ref()
    }
    pub fn attach_application(
        &self,
        application: &crate::production::TerminalApplicationCustody<'_>,
        observer: &mut impl crate::production::PsiCompilationObserver,
    ) -> Result<ProducedTerminalApplication, Vec<Diagnostic>> {
        application.attach(
            self.artifact.view().into(),
            Ok(&self.artifact),
            self.machine_provenance(),
            &mut crate::production::ApplicationReceiptObserver(observer),
        )
    }

    pub fn artifact(&self) -> &Arc<CanonicalTerminalArtifact> {
        &self.artifact
    }
    pub fn build_bindings(&self) -> &TerminalBuildBindings {
        &self.build_bindings
    }
    pub fn build_requests(&self) -> &TerminalBuildRequests {
        &self.build_requests
    }
    pub fn build_filesystem(&self) -> &TerminalBuildFilesystem {
        &self.build_filesystem
    }
    pub fn applications(&self) -> &[Result<ProducedTerminalApplication, Vec<Diagnostic>>] {
        &self.applications
    }
}

#[derive(Clone, Copy, Default)]
pub struct TerminalProductionCustody<'a> {
    pub applications: &'a [crate::production::TerminalApplicationCustody<'a>],
    /// Captured anonymous Build source may be used only in this invocation.
    pub invocation_sources: Option<&'a dyn crate::source_inputs::InvocationSourceConsent>,
}
