#![forbid(unsafe_code)]

//! Compilation requests, retained products, identities, and publication receipts.
//!
//! `compile_request` admits target destinations, invocation policy and product eligibility
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
//! production and batch manifests commit their exact subjects. Native evidence
//! and image receipt construction remain with `native_artifact`; filesystem
//! mechanics remain with `platform_custody`.

mod checked_compilation;
mod compilation_products;
mod compile_outcomes;
pub mod compile_request;
mod evaluated_build;
mod package_build;
mod prepared_build;
mod prepared_source;
pub mod publication;

pub use checked_compilation::{
    CheckedCompilation, CheckedCompilationInputs, CheckedProductReport, RetainedCompilationChecks,
};
pub use compilation_products::{CompileOutputKind, CompileReport};
pub use compile_outcomes::{
    CheckedCompileOutcome, CompileFailure, CompileOutcomes, CompileTargetOutcome,
    CompilerRequestOutcome,
};
pub use compile_request::options::CompileOptions;
pub use compile_request::{
    CheckedCompileRequest, CheckedExecutionRequest, CompileRequest, NativeProductRequest,
    RequestedCompileProduct, SettledCheckReuse,
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
pub use native_artifact::{TerminalRuntimeHostProvision, TerminalRuntimeProvisions};
pub use optimization_core::OptimizationRollbackReceipt;
pub use package_build::PreparedPackageBuild;
pub use package_compilation::RetainedBuildOutputs;
pub use prepared_build::{
    BuiltTypedProgram, CheckedProductSource, PreparedBuild, PreparedProductSource,
};
pub use prepared_source::{
    PreparedCheckedSource, PreparedSourceFrontier, TypedCompilationFrontier,
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

pub use terminal_installation::PrivateEntryRamInitialization;
