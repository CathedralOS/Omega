#![forbid(unsafe_code)]

//! Source checking, compilation, and retained-artifact realization.
//!
//! Start at `compiler.rs`: prepare one `CompileRequest`, then consume its exact
//! returned Build to produce one `CompileReport` per target through checking, Terminal production and
//! native realization. `package_compilation::source_assembly` assembles the physical
//! and generated source set; the folders are the remaining work those steps
//! call: `checked` retains the Build continuation across application-owned
//! execution and checks the product once,
//! `execution_settlement` joins each target's provider installation and
//! selected dispatch against it, `terminal` produces and
//! verifies the Terminal artifact, `native` realizes it for a target, and
//! `product_execution` finishes each requested stop with its permission consent.
//! The product-owned report plan carries the native request and optimization
//! choices through those phases. Finished products and publication belong to the independent
//! `compilation_products` owner; this crate only sequences their production.

mod compiler;
mod execution_settlement;
mod identity;
mod inspection;
pub use terminal_production::ProducedTerminalProgram;
mod product_execution;

#[cfg(test)]
mod product_boundary;

pub use identity::{compiler_build_profile, compiler_identity, compiler_optimization_level};

pub mod checked;
mod native;
mod terminal;

pub use crate::checked::{
    CheckedCompilation, CheckedCompileRequest, CompilationSession, CompletedBuild,
    IndependentComponentDiscovery, IndependentComponentSelection, OptimizationRollback,
    OptimizationRollbackInputError, PackageBuild, PackageBuildInvocation,
    PackageCompilationRequest, PackageCompileFailure, PackageProductOptions,
    PreparedPackageProduct, RestrictedBuildGrants, SourceBuildDeclaration,
    checked_compilation_call_count, preliminary_check_totals, settled_check_totals,
    source_assembly_invocation_count, typed_frontier_reuse_count,
};
pub use build_evaluation::BuildSnapshotRequest;
pub use checked::SourceCompilation;
pub use checked::{PackageReviewFailure, PackageReviewProduct, PackageReviewSource};
pub use compilation_products::{
    CompileFailure, CompileOptions, CompileOutcomes, CompileRequest, CompileTargetOutcome,
    CompilerRequestOutcome, RequestedCompileProduct,
};
pub use compilation_products::{
    CompileOutputKind, CompileReport, ExecutablePublicationReceipt, OptimizationRollbackReceipt,
    RetainedBuildOutputs, RetainedNativeArtifact,
};
pub use compiler::PreparedCompilation;
pub use compiler::package::publish_checked_component;
pub use package_compilation::BuildExecutionInputs;
pub use package_evidence::permission_acceptance::TerminalPermissionAcceptance;
pub use product_execution::{CompilationOutput, CompilationProduct, CompilationProductError};
pub use target::ExplicitTargetSet;
pub use terminal_psi_to_abstract_operations::provider_admission::SourceEvaluatedImportSettlement;
