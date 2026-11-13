/*
    Retained Terminal production outputs (`ProducedTerminalProgram` and
    `PortableTerminalProgram`) and independent application judgment custody.

    We pair canonical or invocation-scoped Terminal code with its source-issued
    Build companions, machine provenance, and per-application root and
    `ProgramEntry` receipts, supporting compiler-storage record persistence and
    late application attachment without reopening source trees.
*/

use crate::TerminalBuildFilesystemCall;
use crate::{TerminalBuildBindings, TerminalBuildFilesystem, TerminalBuildRequests};
use compiler_storage::RecordWriter;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    NativeBindingFormation, ProductDeclarationRequest,
};
use std::sync::Arc;
use terminal_codec::entry_receipt::rejoin_program_entry_receipt;
use terminal_codec::{CanonicalTerminalArtifact, CanonicalTerminalArtifactError};
use terminal_psi::CheckedProgramEntryTerminalReceipt;

#[derive(Debug, PartialEq, Eq, compiler_storage::Record)]
pub struct ProducedTerminalApplication {
    pub receipt: Result<Option<CheckedProgramEntryTerminalReceipt>, Vec<Diagnostic>>,
    pub roots: Vec<Result<crate::TerminalAdmissionRoot, Vec<Diagnostic>>>,
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
    pub(crate) source_records: Vec<typed_trees::SourceRecordCorrespondence>,
}

impl compiler_storage::Record for ProducedTerminalProgram {
    fn write(&self, output: &mut RecordWriter) -> Result<(), compiler_storage::RecordError> {
        output.record(|output| {
            self.artifact()
                .map_err(|error| output.error(error.to_string()))?
                .to_bytes()
                .write(output)?;
            let view = self.view();
            macro_rules! companion {
                ($value:expr) => {
                    $value
                        .ok_or_else(|| output.error("Terminal companion lost its artifact owner"))?
                        .write(output)?
                };
            }
            companion!(self.build_bindings.calls_for(view));
            companion!(self.build_bindings.declarations_for(view));
            companion!(self.build_bindings.queries_for(view));
            companion!(self.build_bindings.native_formations_for(view));
            companion!(self.build_requests.calls_for(view));
            companion!(self.build_filesystem.calls_for(view));
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
            let bytes = Vec::<u8>::read(input)?;
            let artifact = CanonicalTerminalArtifact::from_bytes(&bytes)
                .map_err(|error| input.error(error.to_string()))?;
            let calls: Arc<[crate::TerminalBuildBindingCall]> = Record::read(input)?;
            let declarations: Arc<[Arc<ProductDeclarationRequest>]> = Record::read(input)?;
            let queries: Arc<[crate::TerminalBuildQueryCall]> = Record::read(input)?;
            let native_formations: Arc<[NativeBindingFormation]> = Record::read(input)?;
            let requests: Arc<[crate::TerminalBuildRequestCall]> = Record::read(input)?;
            let filesystem: Arc<[TerminalBuildFilesystemCall]> = Record::read(input)?;
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
    pub fn source_records(&self) -> &[typed_trees::SourceRecordCorrespondence] {
        &self.source_records
    }
    pub fn into_shared_portable(
        self,
    ) -> Result<PortableTerminalProgram, CanonicalTerminalArtifactError> {
        Ok(PortableTerminalProgram {
            artifact: Arc::new(self.program.into_portable()?),
            evidence: self.evidence,
            build_bindings: self.build_bindings,
            build_requests: self.build_requests,
            build_filesystem: self.build_filesystem,
            applications: self.applications,
            provenance: self.provenance,
            source_records: self.source_records,
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
    provenance: terminal_psi::TerminalMachineProvenance,
    source_records: Vec<typed_trees::SourceRecordCorrespondence>,
}

impl PortableTerminalProgram {
    pub const fn evidence(&self) -> Option<&Arc<terminal_codec::EvidenceSeals>> {
        self.evidence.as_ref()
    }
    pub fn source_records(&self) -> &[typed_trees::SourceRecordCorrespondence] {
        &self.source_records
    }
    pub fn attach_application(
        &self,
        application: &crate::TerminalApplicationCustody<'_>,
        observer: &mut impl crate::PsiCompilationObserver,
    ) -> Result<ProducedTerminalApplication, Vec<Diagnostic>> {
        application.attach(
            self.artifact.view().into(),
            Ok(&self.artifact),
            &self.provenance,
            observer,
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
    pub applications: &'a [crate::TerminalApplicationCustody<'a>],
}
