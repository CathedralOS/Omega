//! Retained Terminal code and independent portable application judgments.
//!
//! Portable artifacts and invocation-only code share Terminal semantics, not
//! publication rights. These producer results are not another Psi IR.

use crate::TerminalBuildFilesystemCall;
use crate::{TerminalBuildBindings, TerminalBuildFilesystem, TerminalBuildRequests};
use compiler_storage::RecordWriter;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    NativeBindingFormation, ProductDeclarationRequest,
};
use lowered_psi_to_terminal_psi::rejoin_program_entry_receipt;
use std::sync::Arc;
use terminal_codec::{CanonicalTerminalArtifact, CanonicalTerminalArtifactError};
use terminal_psi::CheckedProgramEntryTerminalReceipt;

/// Source-free inputs for one product's later composition admission. A receipt
/// refusal stays behind the ordered root checks, preserving first-root policy
/// diagnostics without executing product policy inside Psi production.
#[derive(Debug, PartialEq, Eq, compiler_storage::Record)]
pub struct ProducedTerminalApplication {
    pub receipt: Result<Option<CheckedProgramEntryTerminalReceipt>, Vec<Diagnostic>>,
    pub roots: Vec<Result<crate::TerminalAdmissionRoot, Vec<Diagnostic>>>,
}

/// Terminal code beside the checked-source evidence the caller
/// asked production to retain, in request order. Each application carries its
/// own refusal or optional checked `ProgramEntry` receipt. Production success
/// alone does not mean an application was admitted. Earlier representations
/// never escape.
///
/// The retained native route rejoins the checked entry receipt after the
/// checked frontend is gone, so the receipt leaves production beside the
/// artifact instead of remaining a direct-route-only custody object.
/// Build requests and designated operands travel as source-issued companion context,
/// bound to the artifact and exact emitted operations, never as debug payload.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "Terminal production retains independent application judgments"]
pub struct ProducedTerminalProgram {
    pub(super) program: terminal_codec::TerminalProgramProduct,
    pub(super) build_bindings: TerminalBuildBindings,
    pub(super) build_requests: TerminalBuildRequests,
    pub(super) build_filesystem: TerminalBuildFilesystem,
    pub(super) applications: Vec<Result<ProducedTerminalApplication, Vec<Diagnostic>>>,
}

// Source-completion receipts retain portable production. Recover its canonical
// bytes through the ordinary validating ingress, then bind the retained source
// companions to that artifact. Invocation-only code has a live owner and cannot
// cross this boundary. The enclosing source continuation still requires fresh
// input and declaration-authority admission before any receipt can be reused.
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
            self.applications.write(output)
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
            for application in &applications {
                if let Ok(application) = application {
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
            }
            let view = artifact.view().into();
            // Reuse the producer constructors with the recovered companion storage.
            let build_bindings =
                TerminalBuildBindings::new(view, calls, declarations, queries, native_formations);
            let build_requests = TerminalBuildRequests::new(view, requests);
            let build_filesystem = TerminalBuildFilesystem::new(view, filesystem);
            Ok(Self {
                program: terminal_codec::TerminalProgramProduct::Portable(artifact),
                build_bindings,
                build_requests,
                build_filesystem,
                applications,
            })
        })
    }
}

impl ProducedTerminalProgram {
    /// Share portable code and its exact source-issued companions without
    /// copying bytes or recreating companion custody from external ingress.
    pub fn into_shared_portable(
        self,
    ) -> Result<PortableTerminalProgram, CanonicalTerminalArtifactError> {
        Ok(PortableTerminalProgram {
            artifact: Arc::new(self.program.into_portable()?),
            build_bindings: self.build_bindings,
            build_requests: self.build_requests,
            build_filesystem: self.build_filesystem,
            applications: self.applications,
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

    /// Keep only the canonical portable artifact. This discards application
    /// judgments and Build request/binding context, and confers no authority to execute
    /// or publish a native product.
    pub fn into_artifact(
        self,
    ) -> Result<CanonicalTerminalArtifact, CanonicalTerminalArtifactError> {
        self.program.into_portable()
    }

    /// Extract the portable artifact and application judgments, discarding Build
    /// request/binding context. Build consumers borrow it before extraction.
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

/// Portable production storage shared by descriptive consumers. The artifact
/// and source-issued companions retain one identity; no native authority is
/// created by sharing them. Invocation-only code cannot construct this result.
#[derive(Debug)]
pub struct PortableTerminalProgram {
    artifact: Arc<CanonicalTerminalArtifact>,
    build_bindings: TerminalBuildBindings,
    build_requests: TerminalBuildRequests,
    build_filesystem: TerminalBuildFilesystem,
    applications: Vec<Result<ProducedTerminalApplication, Vec<Diagnostic>>>,
}

impl PortableTerminalProgram {
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

/// Independent consumers of one checked execution closure. Admission failure
/// belongs to that application, not to another consumer of the same program.
/// An empty slice requests code without application judgments; the request's
/// publication destination determines whether that code can be exported.
#[derive(Clone, Copy, Default)]
pub struct TerminalProductionCustody<'a> {
    pub applications: &'a [crate::TerminalApplicationCustody<'a>],
}
