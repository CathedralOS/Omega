//! Retained compilation products, identities, and publication receipts.
//!
//! `CompileReport` records which payload a compilation produced and validates
//! its custody joins. It does not choose output paths or install files;
//! `crate::artifact_publication::publish_compilation` owns that operation.
//!
//! `build_outputs` selects completed files from sealed build staging;
//! `terminal_product` retains the portable artifact and native proposal;
//! production and batch manifests commit their exact subjects. `pcc` describes
//! published proof pairs and dispatches receiver checking, independently of
//! producer publication. Native evidence and image receipt construction remain
//! with `native_artifact`; filesystem mechanics remain with `platform_custody`.

mod batch_manifest;
mod build_outputs;
mod compile_report;
mod pcc;
mod production_manifest;
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
    TerminalIntegerComparisonOccurrenceProposal, TerminalNativeBoundaryApplications,
    TerminalNativeOccurrences, TerminalNativeProviders, TerminalNativeRealizationProposal,
};

/// Complete non-clonable Terminal-Psi native artifact retained before output
/// publication. The compatibility name remains while callers migrate from the
/// former legacy `EmissionPlan + EmittedProgram` payload.
pub use native_artifact::NativeArtifact as RetainedNativeArtifact;
