#![forbid(unsafe_code)]

//! Reconciled package compilation inputs and exact source-consumption custody.
//!
//! Start at `package_compilation.rs` for input construction and validation.
//! `source_snapshot` captures the package source a compilation consumed,
//! `source_consumption` records exactly which of it was read, and
//! `semantic_bindings` carries the optional standard-library bindings.
//! `BuildSourceCustody::seal` joins the final checked closure to that source
//! evidence; `CheckedSourceCustody` owns the generated-source export. Neither
//! operation reconstructs package custody in the coordinator, and neither
//! reopens a source file: the frontend read each one once and these records
//! commit to those bytes.
//! `compilation_reuse` owns exact source input comparisons, not admission
//! or scheduling; the compiler consumes those keys without re-projecting them.
//! `source_imports` resolves source frontiers through exact package scopes;
//! `source_assembly` loads and parses them through Psi 00/01, retaining one
//! immutable checkpoint and assembling generated inputs without Build execution.
//! `toolchain_library` locates and validates the compiler's bundled sources.
//! `build_execution` owns immutable build inputs, filesystem custody and
//! output settlement; it consumes interpreter observations, not compiler state.
//! This package is the shared input contract below compilation and package
//! commands, not a dependency on either application's orchestration.

mod anonymous_owner;
pub use anonymous_owner::AnonymousSourceCapture;
pub mod build_execution;
mod canonical_fs;
mod checked_source_cache;
mod codec_bytes;
mod compilation_reuse;
mod cursor;
pub(crate) use optimization_core::digest_encoder;
pub mod source_assembly;
pub mod source_imports;
pub mod toolchain_library;
pub use build_execution::*;
mod package_compilation;
mod review_cache_digests;
#[doc(hidden)]
pub mod review_record_encoding;
mod semantic_bindings;
mod source_consumption;
mod source_snapshot;
mod target_profile_tags;
mod wire_codec_sources;

pub use build_declarations::BuildDeclarationKind;
pub use checked_source_cache::{CheckedSourceCache, CheckedSourceCacheLimits};
pub use compilation_reuse::SourceAssemblyInputs;
pub use package_compilation::{
    BuildDependencyOccurrence, IndependentComponentDescription, PackageCompilationInputError,
    PackageCompilationInputs, PackageCompilationSourceInputs, PackageCompilationTargetInputs,
    PackageDependencyBinding, PackageDependencyClosure, PackageGeneratedSourceBundle,
    PackageSourceBinding,
};
pub use review_cache_digests::{review_cache_inputs_digest, review_cache_snapshot_digest};
pub use semantic_bindings::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, accepted_service_schema_digest,
    program_entry_semantic_binding_role,
};
pub use source_consumption::{
    BuildSourceCustody, CheckedSourceCustody, ConsumedSourceUnit, ConsumedSourceUnitKind,
    PackageSourceConsumptionCommitment, derive_package_compilation_subject,
    derive_source_consumption_commitment, toolchain_source_identities,
    toolchain_source_identity_digest,
};
pub(crate) use source_snapshot::capture_scoped_source_input;
pub use source_snapshot::{
    BuildSourceCaptureObligation, BuildSourceCaptureRequest, capture_package_source_input,
};
