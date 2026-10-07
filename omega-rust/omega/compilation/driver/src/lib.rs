#![forbid(unsafe_code)]

//! Source checking, compilation, and retained-artifact realization.
//!
//! Start at `compiler.rs`: one `CompileRequest` becomes one `CompileReport`
//! per target through source preparation, checking, Terminal production and
//! native realization. `package_compilation::source_assembly` assembles the physical
//! and generated source set; the folders are the remaining work those steps
//! call: `checked` evaluates the build and settles checked Psi, `terminal` produces and
//! verifies the Terminal artifact, `native` realizes it for a target, and
//! finished products and publication belong to the independent
//! `compilation_products` owner; this crate only sequences their production.

mod compiler;
mod identity;

pub use identity::compiler_identity;

pub mod checked;
pub mod native;
pub mod terminal;

pub use crate::checked::{
    CheckedCompilation, CheckedCompileRequest, IndependentComponentDiscovery,
    IndependentComponentSelection, OptimizationRollback, OptimizationRollbackInputError,
    PackageBuildOutcome, PreparedCheckedSource, PreparedPackageBuild, RestrictedBuildGrants,
    SettledCheckReuse, check_prepared_package_build, checked_compilation_call_count,
    compile_checked, compile_checked_reusing_builds, compile_to_checked, preliminary_check_totals,
    prepare_package_build, settled_check_totals, source_assembly_invocation_count,
    typed_frontier_reuse_count,
};
pub use crate::native::realize_retained_native_artifact;
pub use build_evaluation::BuildSnapshotRequest;
pub use compilation_products::RetainedNativeRealizationRequest;
pub use compilation_products::{
    CompileDestination, CompileOptions, CompileOutcomes, CompileRequest, CompileTargetOutcome,
    CompilerRequestOutcome, RequestedCompileProduct,
};
pub use compilation_products::{
    CompileOutputKind, CompileReport, ExecutablePublicationReceipt, OptimizationRollbackReceipt,
    RetainedBuildOutputs, RetainedNativeArtifact,
};
pub use compiler::compile;
pub use compiler::package::retained_terminal_artifact_from_checked_package;
pub use target::ExplicitTargetSet;
pub use terminal_psi_to_abstract_operations::provider_admission::SourceEvaluatedImportSettlement;
