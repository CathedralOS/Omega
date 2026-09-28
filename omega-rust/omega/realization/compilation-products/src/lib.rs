#![forbid(unsafe_code)]

//! Retained compilation products, identities, and publication receipts.
//!
//! `CompileReport` records which payload a compilation produced and validates
//! its custody joins. It does not choose output paths or install files;
//! `crate::publication::publish_compilation` owns that operation.
//!
//! `package_compilation` retains and publishes completed build files;
//! the Terminal entry stage retains the portable artifact and native proposal;
//! production and batch manifests commit their exact subjects. `pcc` describes
//! published proof pairs and dispatches receiver checking, independently of
//! producer publication. Native evidence and image receipt construction remain
//! with `native_artifact`; filesystem mechanics remain with `platform_custody`.

mod batch_manifest;
mod compilation_products;
mod pcc;
mod production_manifest;
pub mod publication;

pub use batch_manifest::{
    BatchChildCommitment, BatchChildOutcome, BatchChildRow, BatchCompilationManifest,
    BatchCompilationManifestIdentity,
};
pub use compilation_products::{CompileOutputKind, CompileReport};
pub use native_artifact::{
    ExecutableContainerDigest, ExecutableInstallationEvidenceDigest, ExecutablePublicationReceipt,
    NativePublicationCertificateDigest, NativePublicationEvidenceDigest,
    executable_installation_evidence_digest,
};
pub use native_artifact::{NativePackagePublicationReceipt, PackagePublicationComponent};
pub use optimization_core::OptimizationRollbackReceipt;
pub use package_compilation::{ProductionCompilationSubject, RetainedBuildOutputs};
pub use pcc::{PccPublicationReceipt, verify_published_proof_pair};
pub use production_manifest::{
    FinalRealizationEvidenceError, ProductionArtifactIdentity, ProductionCompilationManifest,
    ProductionCompilationManifestIdentity,
};
pub use terminal_psi_to_abstract_operations::{
    RetainedTerminalArtifact, TerminalCallbackOccurrenceProposal, TerminalCallbackThunkArtifact,
    TerminalCompilerBuiltinProposal, TerminalIeeeFloatComparisonOccurrenceProposal,
    TerminalIntegerComparisonOccurrenceProposal, TerminalNativeBoundaryApplications,
    TerminalNativeOccurrences, TerminalNativeProviders, TerminalNativeRealizationProposal,
};

/// Complete non-clonable Terminal-Psi native artifact retained before output
/// publication. The compatibility name remains while callers migrate from the
/// former legacy `EmissionPlan + EmittedProgram` payload.
pub use native_artifact::NativeArtifact as RetainedNativeArtifact;
