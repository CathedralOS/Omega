/*
We coordinate Omega compilation from assembled source units and Build
continuations through Checked Psi, target-selected provider and entry
settlement, canonical Terminal Psi production, and native artifact realization.

Start at `target_batch.rs` for batch compilation across target profiles and
`checked.rs` for session-owned source and package compilation. A compilation
begins by validating target requests and preparing the shared source and Build
continuation (`begin_compilation`). Once the application steps the Build
activation and returns its result, `complete_compilation` checks the Product
source once, captures each target's declarations (`checked/target_preparation.rs`),
settles provider plans and entry bindings (`execution_settlement.rs`), batches
portable Terminal Psi production (`terminal.rs`), and finishes each requested
stop (`product_execution.rs`).
*/

/*
We separate compilation sequencing from product custody (`compilation_products`)
and target-neutral language semantics (`psi_compiler`). This crate never
executes Build effects directly or reinterprets Checked Psi outside the stage
owners; instead, we hold the opaque continuation across application-driven Build
stepping and route each settled target through its exact stop (`Check`,
`Terminal`, `Native`, `Inspection`, or `Component`).

We also embed the loaded compiler's build-input identity (`compiler_identity`,
`compiler_input_identity`, and `compiler_optimization_level`) from `OUT_DIR` at
compile time so package and component caches can verify that the running
compiler binary and bundled toolchain library match retained artifacts without
trusting mutable filesystem paths or runtime environment variables.
*/

#![forbid(unsafe_code)]

mod execution_settlement;
mod observations;
#[cfg(test)]
mod product_boundary;
mod product_execution;
mod target_batch;
mod terminal;

pub mod checked;

pub use crate::checked::{
    BuildAdvance, CheckedCompilation, CheckedCompileRequest, CompilationSession, CompletedBuild,
    CurrentCheckerVerdict, IndependentComponentDiscovery, IndependentComponentSelection,
    OptimizationRollback, OptimizationRollbackInputError, PackageBuild, PackageBuildInvocation,
    PackageCompilationRequest, PackageCompileFailure, PackageProductOptions,
    PackageProductSourceRequest, PackageSourceRequest, PreparedPackageProduct,
    RestrictedBuildGrants, SourceBuildDeclaration, SourceCompilation, TypedFrontier,
    TypedFrontierCheck, TypedFrontierRequest, checked_compilation_call_count,
    preliminary_check_totals, settled_check_totals, source_assembly_invocation_count,
    typed_frontier_reuse_count,
};
pub use build_settings::BuildSnapshotRequest;
pub use checked::checking::ProductQueries;
#[doc(hidden)]
pub use checked::checking::build_continuation::{
    core_product_speculations, refuse_core_product_speculation,
};
pub use checked::{PackageReviewFailure, PackageReviewProduct, PackageReviewSource};
pub use compilation_products::{
    CompileFailure, CompileOptions, CompileOutcomes, CompileOutputKind, CompileReport,
    CompileRequest, CompileTargetOutcome, CompilerRequestOutcome, ExecutablePublicationReceipt,
    OptimizationRollbackReceipt, RequestedCompileProduct, RetainedBuildOutputs,
    RetainedNativeArtifact,
};
pub use native_compiler::SourceEvaluatedImportSettlement;
pub use package_compilation::BuildExecutionInputs;
pub use package_evidence::permission_acceptance::TerminalPermissionAcceptance;
pub use product_execution::{
    CompilationOutput, CompilationProduct, CompilationProductError, TargetProductOutcome,
};
pub use psi_compiler::{
    LibrarySliceTotals, ProducedTerminalProgram, declaration_page_fork_totals,
    dependency_body_read_totals, dependency_entailment_totals, library_slice_totals,
    retained_layout_match_hits, whole_program_reuse_hits,
};
pub use target::ExplicitTargetSet;
pub use target_batch::PreparedCompilation;

pub fn compiler_input_identity() -> Result<[u8; 32], String> {
    let library = package_compilation::toolchain_library::identity()?;
    let mut digest = optimization_core::digest_encoder::DomainDigestEncoder::new(
        b"omega-accepted-compiler-inputs-v1\0",
    );
    digest.raw(&compiler_identity());
    digest.raw(library.as_bytes());
    Ok(digest.finish())
}

pub const fn compiler_identity() -> [u8; 32] {
    *include_bytes!(concat!(env!("OUT_DIR"), "/compiler-identity.bin"))
}

pub const fn compiler_optimization_level() -> &'static str {
    env!("OMEGA_COMPILER_OPT_LEVEL")
}
