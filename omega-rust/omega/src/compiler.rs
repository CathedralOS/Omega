#![forbid(unsafe_code)]

//! Source checking, compilation, and retained-artifact realization.
//!
//! Start at `compiler.rs`: one `CompileRequest` becomes one `CompileReport`
//! per target through source preparation, checking, Terminal production and
//! native realization. The folders are the work those steps call:
//! `sources` assembles the physical and generated source set, `checked`
//! evaluates the build and settles checked Psi, `terminal` produces and
//! verifies the Terminal artifact, `native` realizes it for a target, and
//! `report` owns the compile report, publication receipts and proof sidecars.

mod compiler;

pub mod checked;
pub mod native;
pub mod report;
pub mod sources;
pub mod terminal;

pub use self::compiler::compile;
pub use self::compiler::options::CompileOptions;
pub use self::compiler::package::{
    published_independent_component_description, retained_terminal_report_from_checked_package,
};
pub use self::compiler::request::{
    CompileOutcomes, CompileRequest, CompileTargetOutcome, ExplicitTargetSet,
    RequestedCompileProduct, TargetCompileConfiguration,
};
pub use crate::build_evaluation::BuildSnapshotRequest;
pub use crate::compiler::checked::{
    CheckedAdmission, CheckedCompilation, CheckedCompileRequest, IndependentComponentDiscovery,
    IndependentComponentSelection, OptimizationRollback, OptimizationRollbackInputError,
    PreparedCheckedSource, RestrictedBuildGrants, admit_checked_compilation, compile_to_checked,
};
pub use crate::compiler::native::{
    RetainedNativeRealizationRequest, SourceEvaluatedImportSettlement,
    realize_retained_native_artifact,
};
pub use crate::compiler::report::{
    BatchChildCommitment, BatchChildOutcome, BatchChildRow, BatchCompilationManifest,
    BatchCompilationManifestIdentity, CompileOutputKind, CompileReport,
    ExecutablePublicationReceipt, FinalRealizationEvidenceError, OptimizationRollbackReceipt,
    ProductionArtifactIdentity, ProductionCompilationManifest,
    ProductionCompilationManifestIdentity, ProductionCompilationSubject, RetainedBuildOutputs,
    RetainedNativeArtifact,
};
pub use crate::compiler::terminal::{
    validate_lowered_ieee_float_comparison_custody, validate_lowered_integer_comparison_custody,
};
pub use crate::trust_model::{TrustAdmission, TrustAdmissionSettlement};
