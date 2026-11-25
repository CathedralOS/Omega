//! Source-issued Build operations and application judgments travel with the checked code.
//! Portable publication seals the declaration owners and evidence into the artifact.

use crate::declaration_selection::NativeBindingFormation;
use crate::declaration_selection::ProductDeclarationRequest;
use crate::ProducedTerminalApplication;
use crate::TerminalBuildFilesystemCall;
use crate::{TerminalBuildBindings, TerminalBuildFilesystem, TerminalBuildRequests};
use diagnostics::Diagnostic;
use psi_authorization::selection_questions::ProductSchemaCustody;
use record_storage::RecordWriter;
use std::sync::Arc;
use terminal_codec::entry_receipt::rejoin_program_entry_receipt;
use terminal_codec::{CanonicalTerminalArtifact, CanonicalTerminalArtifactError};

// Check the format before decoding a call's operation and typed Build operand.
const PRODUCED_PROGRAM_SCHEMA: u64 = u64::from_le_bytes(*b"OMGBLD03");

#[cfg(test)]
mod schema_tests {
    #[test]
    fn old_companion_rows_refuse_before_artifact_decoding() {
        for bytes in [
            record_storage::encode(&b"PSITERM\0".to_vec(), Default::default()).unwrap(),
            record_storage::encode(&u64::from_le_bytes(*b"OMGBLD02"), Default::default()).unwrap(),
        ] {
            let error =
                record_storage::decode::<super::ProducedTerminalProgram>(&bytes, Default::default())
                    .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("unsupported produced program record schema"),
                "{error}"
            );
        }
    }
}

// An emitted call site does not authenticate its saved source action or Product
// operands. These records need their accepted source correspondence rejoined.
fn require_saved_build_correspondence(
    bindings: &[crate::TerminalBuildBindingCall],
    declarations: &[Arc<ProductDeclarationRequest>],
    queries: &[crate::TerminalBuildQueryCall],
    native_formations: &[NativeBindingFormation],
    requests: &[crate::TerminalBuildRequestCall],
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
    pub(crate) provenance: psi_publication_records::TerminalMachineProvenance,
    pub(crate) source_records: Vec<crate::source_state::SourceRecordCorrespondence>,
    pub(crate) source_selections: Option<Arc<crate::TerminalSourceSelections>>,
}

impl record_storage::Record for ProducedTerminalProgram {
    fn write(&self, output: &mut RecordWriter) -> Result<(), record_storage::RecordError> {
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
        input: &mut record_storage::RecordReader<'_, Context>,
    ) -> Result<Self, record_storage::RecordError> {
        use record_storage::Record;
        input.record(|input| {
            if u64::read(input)? != PRODUCED_PROGRAM_SCHEMA {
                return Err(input.error("unsupported produced program record schema"));
            }
            let bytes = Vec::<u8>::read(input)?;
            let artifact = CanonicalTerminalArtifact::from_bytes(&bytes)
                .map_err(|error| input.error(error.to_string()))?;
            let calls: Arc<[crate::TerminalBuildBindingCall]> = Record::read(input)?;
            let declarations: Arc<[Arc<ProductDeclarationRequest>]> = Record::read(input)?;
            let queries: Arc<[crate::TerminalBuildQueryCall]> = Record::read(input)?;
            let native_formations: Arc<[NativeBindingFormation]> = Record::read(input)?;
            let requests: Arc<[crate::TerminalBuildRequestCall]> = Record::read(input)?;
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
            crate::build_bindings::validate_companions(
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
    pub fn source_selections(&self) -> Option<&Arc<crate::TerminalSourceSelections>> {
        self.source_selections.as_ref()
    }
    /// Seal source publication metadata into the artifact. Each bound application's
    /// own receipt and root checks travel separately to the caller.
    pub fn into_portable_publication(
        self,
    ) -> Result<
        (
            CanonicalTerminalArtifact,
            Vec<Result<ProducedTerminalApplication, Vec<Diagnostic>>>,
        ),
        CanonicalTerminalArtifactError,
    > {
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
        let entry_metadata = super::published_entry::issue(
            &artifact,
            owners.as_ref(),
            self.provenance,
            !self.applications.is_empty(),
        );
        let artifact = artifact.with_publication(Arc::new(entry_metadata), self.evidence)?;
        Ok((artifact, self.applications))
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
    pub applications: &'a [crate::TerminalApplicationCustody<'a>],
    /// Captured anonymous Build source may be used only in this invocation.
    pub invocation_sources: Option<&'a dyn crate::source_inputs::InvocationSourceConsent>,
}
