//! Complete package-manager operations composed from lower-level owners.
//!
//! [`prepare_local_project`] is the ordinary compiler entrance for a project
//! with `build.omg`. [`inspect_source`] acquires and inspects one source without
//! admitting it. [`recover_locked_sources`] reacquires a retained exact source
//! graph without reusing its policy as fresh review. [`check_locked_sources`]
//! freshly checks it and reports complete policy changes. [`review_package_change`]
//! checks an install/update candidate and joins its comparison and project
//! decisions to a proposed source lock section. [`stage_build_dependency_edit`]
//! prepares a declaration edit without changing live source. File transactions belong beside
//! these operations rather than in the command-line binary.

mod check_locked_sources;
mod compile_project;
pub mod inspect_source;
mod package_change;
mod prepare_project;
mod recover_locked_sources;
mod stage_build_edit;

pub use check_locked_sources::{
    CheckLockedSourcesError, CheckedLockedSources, check_locked_sources,
};
pub use compile_project::{
    CompilePreparedLocalProjectNativeError, LocalProjectRootPolicy,
    PreparedLocalProjectNativeRequest, compile_prepared_local_project_for_native,
};
pub use inspect_source::{
    PackageSourceInspection, PackageSourceInspectionError, PackageSourceRequest,
    PackageSourceRequestParseError, SourceAdapter, inspect_package_source,
    inspect_package_source_locator,
};
pub use package_change::{PackageChangeError, PackageChangeReview, review_package_change};
pub use prepare_project::{PrepareLocalProjectError, PreparedLocalProject, prepare_local_project};
pub use recover_locked_sources::{
    LockedSourceRecoveryOptions, RecoverLockedSourcesError, recover_locked_sources,
};
pub use stage_build_edit::stage_build_dependency_edit;
