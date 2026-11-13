/*
We orchestrate compiler work from assembled syntax and Build continuations to a
sealed `CheckedCompilation` and target-captured declaration custody.

Start at `CompilationSession` (`compilation_session.rs`) for invocation-level
reuse of parsed sources, Pure Build judgments, and checked trees, and
`checking.rs` for the feed-forward source and Build continuation pipeline.
*/

/*
Each source compilation prepares its Build activation, rejoins the application's
returned Build, resolves and types generated source against the retained base,
and checks and seals `CheckedProductSource` (`checking/source_checking.rs`).
Each requested target then captures its declarations (`target_preparation.rs`)
and settles providers, dispatch, entry bindings, and task activations without
reopening source checking.
*/

pub(crate) mod checked_slice_store;
pub(crate) mod checking;
mod compilation_session;
mod component_cache;
mod package_compilation;
mod package_review;
mod product_batch;
mod target_preparation;
mod test_roots;

pub use artifacts::compile_timings::counters::{
    checked_compilation_call_count, preliminary_check_totals, settled_check_totals,
    source_assembly_invocation_count, typed_frontier_reuse_count,
};
pub use checking::compile_thread::{
    COMPILE_STACK_SIZE, run_on_compile_thread, spawn_compile_worker,
};
pub use checking::typed_frontier::{
    CurrentCheckerVerdict, TypedFrontier, TypedFrontierCheck, TypedFrontierRequest,
};
pub use checking::{
    CheckedCompilation, CheckedCompileOutcome, CheckedCompileRequest,
    IndependentComponentDiscovery, IndependentComponentSelection, PreparedCheckedSource,
    PreparedPackageBuild, RestrictedBuildGrants, SourceCompilation,
};
pub use compilation_session::CompilationSession;
pub use optimization_core::{
    OptimizationRollback, OptimizationRollbackInputError, OptimizationRollbackSettlement,
};
pub use package_compilation::{
    CompletedBuild, PackageBuild, PackageBuildInvocation, PackageCompilationRequest,
    PackageCompileFailure, PackageProductOptions, PackageProductSourceRequest,
    PackageSourceRequest, PreparedPackageProduct, SourceBuildDeclaration,
};
pub use package_review::{PackageReviewFailure, PackageReviewProduct, PackageReviewSource};
pub(crate) use target_preparation::{PreparedProductTarget, PreparedTargetDeclarations};
