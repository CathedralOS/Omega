//! Compiler orchestration from assembled syntax to one sealed checked compilation.
//!
//! Start at `CompilationSession` for invocation ownership and `checking.rs`
//! for the feed-forward operation. It evaluates the Build, resolves and types
//! generated source against the retained base, then checks and seals custody.
//! Each target then settles providers, dispatch, entry bindings and task
//! activations without reopening checking. The `checking` folder holds the build
//! continuation through the Psi facade; `compilation_products` owns the sealed
//! carrier and its projections.
//! The checked product settles trust through the realization trust owner;
//! Build evaluation owns declaration admission before any Build effects.
//! Checked-product selection executes in Psi stage 04. Exact-rule rollback
//! settlement is owned by `optimization_core`.

pub(crate) mod checking;
mod compilation_session;
pub use compilation_session::CompilationSession;
mod package_compilation;
mod package_review;
pub use package_review::{PackageReviewFailure, PackageReviewProduct, PackageReviewSource};
mod product_batch;
pub use package_compilation::{
    PackageBuild, PackageBuildInvocation, PackageCompilationRequest, PackageCompileFailure,
    PreparedPackageProduct,
};

pub use artifacts::compile_timings::counters::{
    checked_compilation_call_count, preliminary_check_totals, settled_check_totals,
    source_assembly_invocation_count, typed_frontier_reuse_count,
};
pub use checking::compile_thread::run_on_compile_thread;
pub use checking::{
    CheckedCompilation, CheckedCompileOutcome, CheckedCompileRequest,
    IndependentComponentDiscovery, IndependentComponentSelection, PreparedCheckedSource,
    PreparedPackageBuild, RestrictedBuildGrants, SettledCheckReuse,
};
pub(crate) use checking::{check_product_source, prepare_product_source};
pub use optimization_core::{
    OptimizationRollback, OptimizationRollbackInputError, OptimizationRollbackSettlement,
};
