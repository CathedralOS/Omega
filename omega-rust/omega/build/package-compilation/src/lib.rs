#![forbid(unsafe_code)]

//! Reconciled package compilation inputs and exact source-consumption custody.
//!
//! Start at `package_compilation.rs` for input construction and validation.
//! `source_snapshot` captures the package source a compilation consumed,
//! `source_consumption` records exactly which of it was read, and
//! `semantic_bindings` carries the optional standard-library bindings.

mod package_compilation;
mod semantic_bindings;
mod source_consumption;
mod source_snapshot;

pub use build_declarations::BuildDeclarationKind;
pub use package_compilation::{
    BuildDependencyOccurrence, IndependentComponentDescription, PackageCompilationInputError,
    PackageCompilationInputs, PackageCompilationSourceInputs, PackageCompilationTargetInputs,
    PackageDependencyBinding, PackageDependencyClosure, PackageGeneratedSourceBundle,
    PackageSourceBinding,
};
pub use semantic_bindings::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, accepted_service_schema_digest,
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
