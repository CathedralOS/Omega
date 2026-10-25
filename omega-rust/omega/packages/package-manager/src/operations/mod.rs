//! Complete package-manager operations composed from lower-level owners.
//!
//! [`prepare_local_project`] is the ordinary compiler entrance for a project
//! with `build.omg`. [`inspect_source`] acquires and inspects one source without
//! admitting it. [`review_package_change`]
//! checks an install/update candidate and joins its comparison and project
//! decisions to a proposed source lock section. [`stage_build_dependency_edit`]
//! prepares a declaration edit without changing live source. File transactions belong beside
//! these operations rather than in the command-line binary.

mod accepted_compilation;
pub(crate) use accepted_compilation::compiler_input_identity;
mod check_project;
mod compile_project;
mod inspect_packages;
pub mod inspect_source;
mod package_change;
mod prepare_project;
mod publication;
mod stage_build_edit;

pub use check_project::{
    CheckPreparedLocalProjectError, PreparedLocalProjectCheckRequest,
    check_prepared_local_project_for_targets, inspect_prepared_local_project,
};
pub use compile_project::{
    CompilePreparedLocalProjectNativeError, PreparedLocalProjectNativeRequest,
    compile_prepared_local_project_for_native, compile_prepared_local_project_for_native_targets,
};
pub use inspect_packages::{PackageInspectionOptions, inspect_packages};
pub(crate) use inspect_source::PackageSourceRequest;
pub use inspect_source::{SourceAdapter, inspect_package_source_locator};
pub(crate) use package_change::{
    PackageChangeError, PackageChangeReview, review_package_change_reusing,
};
pub(crate) use prepare_project::LOCAL_PROJECT_CONTEXT;
pub use prepare_project::{
    LocalProjectPreparationOptions, PrepareLocalProjectError, PreparedLocalProject,
    prepare_local_project, prepare_local_project_for_targets,
};
pub(crate) use publication::{
    PackageFileTransaction, PackagePublicationError, PackagePublicationLimits,
    publish_reviewed_package_change,
};
pub(crate) use stage_build_edit::stage_build_dependency_edit;
