//! Product reports, requested file publication, and their custody joins.
//!
//! Start at `CompileReport` in `compile_report.rs`: select the product,
//! prepare its proof companions and native evidence, install the requested
//! files, then retain their receipts. The report checks product-specific
//! payload cardinality and the joins between package, Terminal and native
//! evidence; it does not compute native publication digests.
//!
//! `native_artifact::PreparedExecutablePublication` owns native evidence
//! preparation and receipt construction. Its receipt replays installation
//! identity and published-file bytes. These custody commitments do not
//! establish native behavior or receiving authority.
//!
//! `build_outputs.rs` selects completed files from sealed build staging.
//! `publication.rs` prepares proof pairs and their companion paths;
//! `platform_custody::file_publication` stages, installs and replays exact bytes.
//! A failure removes newly installed members, reports cleanup failures, and
//! preserves untouched destinations; replaced files are not restored.
//!
//! Publication consumes a finished report, not another compiler request.
//! Every report constructor checks custody before returning its result.
//!
//! A selected macOS GUI product publishes one complete `.app` package through
//! `native_artifact::publish_macos_application_package` — the executable, the fixed
//! `Contents/Info.plist`, the exact directory shape, and the three-way
//! identifier join specified in `wiki/spec/build/macos_application.md` —
//! instead of the flat executable path. The retained authored
//! `builder.application` name supplies the basename and inner leaf; the
//! retained `builder.identifier` must agree with the CodeDirectory identity
//! already bound into the signed bytes.
//!
//! Start at [`CompileReport`] in `compile_report.rs` for product selection,
//! publication sequencing, and custody validation. Follow
//! `publication/psi_pair.rs` for preparing and staging the same
//! Psi artifact/proof pair under Terminal, flat native, and package publication.
//! `pcc.rs` owns pair receipts and dispatches receiver checking by product kind;
//! native evidence construction and replay live in `native_artifact::proof`
//! (`backend/artifacts/native-artifact/src/proof.rs`). Publication policy stays
//! here; the native artifact crate consumes source-free bytes and evidence.

mod batch_manifest;
mod build_outputs;
mod compile_report;
mod pcc;
mod production_manifest;
mod publication;
mod terminal_product;

pub use batch_manifest::{
    BatchChildCommitment, BatchChildOutcome, BatchChildRow, BatchCompilationManifest,
    BatchCompilationManifestIdentity,
};
pub use build_outputs::RetainedBuildOutputs;
pub use compile_report::{CompileOutputKind, CompileReport};
pub use native_artifact::{
    ExecutableContainerDigest, ExecutableInstallationEvidenceDigest, ExecutablePublicationReceipt,
    NativePublicationCertificateDigest, NativePublicationEvidenceDigest,
    executable_installation_evidence_digest,
};
pub use native_artifact::{NativePackagePublicationReceipt, PackagePublicationComponent};
pub use optimization_core::OptimizationRollbackReceipt;
pub use pcc::{PccPublicationReceipt, verify_published_proof_pair};
pub use production_manifest::{
    FinalRealizationEvidenceError, ProductionArtifactIdentity, ProductionCompilationManifest,
    ProductionCompilationManifestIdentity, ProductionCompilationSubject,
};
pub use terminal_product::{
    RetainedTerminalArtifact, TerminalCallbackOccurrenceProposal, TerminalCallbackThunkArtifact,
    TerminalCompilerBuiltinProposal, TerminalIeeeFloatComparisonOccurrenceProposal,
    TerminalIntegerComparisonOccurrenceProposal, TerminalNativeRealizationInputs,
    TerminalNativeRealizationProposal,
};

/// Complete non-clonable Terminal-Psi native artifact retained before output
/// publication. The compatibility name remains while callers migrate from the
/// former legacy `EmissionPlan + EmittedProgram` payload.
pub use native_artifact::NativeArtifact as RetainedNativeArtifact;
