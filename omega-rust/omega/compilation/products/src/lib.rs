#![forbid(unsafe_code)]

//! Compilation requests, retained products, identities, and publication receipts.
//!
//! `compile_request` validates target configurations and product eligibility
//! without executing a compiler phase. `compile_outcomes` retains every result,
//! classifies each as the compiler request's `Complete`, `Reject` or
//! `Incomplete` outcome, and binds its batch commitment; the compiler owns the
//! target execution loop.
//! `prepared_source` retains exact parsed/typed work and selects reuse without
//! invoking assembly, resolution, typing or Build execution.
//! `prepared_build` retains completed typed Build continuations by exact inputs;
//! the coordinator runs cache misses and each target's subsequent checking.
//! `evaluated_build` retains Build settings and observation results without
//! depending on their evaluator. `checked_compilation` seals whole checked
//! stage products against package source custody, supplies their projections and
//! requests explicit trust settlement from `trust_model`;
//! it does not execute stages or confer package/native admission.
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
mod checked_compilation;
mod compilation_products;
mod compile_outcomes;
pub mod compile_request;
mod evaluated_build;
mod pcc;
mod prepared_build;
mod prepared_source;
mod production_manifest;
pub mod publication;

pub use batch_manifest::{
    BatchChildCommitment, BatchChildOutcome, BatchChildRow, BatchCompilationManifest,
    BatchCompilationManifestIdentity,
};
pub use checked_compilation::{
    CheckedCompilation, CheckedCompilationInputs, CheckedProductReport, CompileReportSettlement,
    validate_lowered_ieee_float_comparison_custody, validate_lowered_integer_comparison_custody,
};
pub use compilation_products::{CompileOutputKind, CompileReport};
pub use compile_outcomes::{
    CheckedCompileOutcome, CompileOutcomes, CompileTargetOutcome, CompilerRequestOutcome,
};
pub use compile_request::options::CompileOptions;
pub use compile_request::{
    CheckedCompileRequest, CheckedExecutionRequest, CompileRequest, NativeProductRequest,
    NativeRealizationRequest, RequestedCompileProduct, RequestedNativeArtifactError,
    RetainedNativeRealizationRequest, TargetCompileConfiguration,
};
pub use evaluated_build::{
    BuildAdmission, BuildConfig, ComputedBuildConfig, IndependentComponentDiscovery,
    IndependentComponentSelection,
};
pub use native_artifact::{
    ExecutableContainerDigest, ExecutableInstallationEvidenceDigest, ExecutablePublicationReceipt,
    NativePublicationCertificateDigest, NativePublicationEvidenceDigest,
    executable_installation_evidence_digest,
};
pub use native_artifact::{NativePackagePublicationReceipt, PackagePublicationComponent};
pub use optimization_core::OptimizationRollbackReceipt;
pub use package_compilation::{ProductionCompilationSubject, RetainedBuildOutputs};
pub use pcc::{PccPublicationReceipt, verify_published_proof_pair};
pub use prepared_build::{
    BuildEvaluationAccounting, BuiltTypedProgram, PreparedBuild, PreparedBuilds,
};
pub use prepared_source::{
    PreparedCheckedSource, PreparedSourceFrontier, TypedCompilationFrontier, TypedFrontierRetention,
};
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
