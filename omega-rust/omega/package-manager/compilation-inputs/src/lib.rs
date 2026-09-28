#![forbid(unsafe_code)]

//! Reconciled package compilation inputs and exact source-consumption custody.
//!
//! Start at `package_compilation.rs` for input construction and validation.
//! `production_subject` validates and encodes package/source/build/target custody
//! without depending on artifact producers or compiler reports.
//! `source_snapshot` captures the package source a compilation consumed,
//! `source_consumption` records exactly which of it was read, and
//! `semantic_bindings` carries the optional standard-library bindings.
//! `compilation_reuse` owns exact source/build input comparisons, not admission
//! or scheduling; the compiler consumes those keys without re-projecting them.
//! `source_imports` resolves source frontiers through exact package scopes;
//! `toolchain_library` locates and validates the compiler's bundled sources.
//! `build_execution` owns immutable build inputs, filesystem custody and
//! output settlement; it consumes interpreter observations, not compiler state.
//! This package is the shared input contract below compilation and package
//! commands, not a dependency on either application's orchestration.

pub mod build_execution;
mod checked_source_cache;
mod compilation_reuse;
pub mod source_imports;
pub mod toolchain_library;
pub use build_execution::*;
mod package_compilation;
mod production_subject;
mod semantic_bindings;
mod source_consumption;
mod source_snapshot;

pub use build_declarations::BuildDeclarationKind;
pub use checked_source_cache::{
    CachedSourceIndex, CheckedSourceCache, CheckedSourceCacheLimits, CheckedSourceCacheOutcome,
};
pub use compilation_reuse::{BuildExecutionInputs, SourceAssemblyInputs};
pub use package_compilation::{
    BuildDependencyOccurrence, IndependentComponentDescription, PackageCompilationInputError,
    PackageCompilationInputs, PackageCompilationSourceInputs, PackageCompilationTargetInputs,
    PackageDependencyBinding, PackageDependencyClosure, PackageGeneratedSourceBundle,
    PackageSourceBinding,
};
pub use production_subject::ProductionCompilationSubject;
pub use semantic_bindings::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, accepted_service_schema_digest,
    program_entry_semantic_binding_role,
};
pub use source_consumption::{
    ConsumedSourceUnit, ConsumedSourceUnitKind, PackageCompilationSubject,
    PackageSourceConsumptionCommitment, derive_consumed_source_units,
    derive_package_compilation_subject, derive_source_consumption_commitment,
    toolchain_source_identities, toolchain_source_identity_digest, verify_current_files,
};
pub use source_snapshot::{
    BuildSourceCaptureObligation, BuildSourceCaptureRequest, capture_package_source_input,
    capture_scoped_source_input,
};
