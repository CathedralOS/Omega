/*
Complete package-manager operations composed from lower-level source, review, and lock owners.

`begin_local_project_preparation` admits a local project with `build.omg`, and
`admit_project_compilation` returns the compiler inputs and custody without executing
them so application Build owns ordinary compilation scheduling. `inspect_source`
acquires and inspects a single source without admitting it, `review_package_change`
checks an install/update candidate and joins its comparison and project decisions to
a proposed source lock section, and `stage_build_dependency_edit` prepares a declaration
edit on a candidate snapshot without mutating the live checkout.
*/

mod accepted_compilation;
pub(crate) use accepted_compilation::compiler_input_identity;
mod inspect_packages;
pub mod inspect_source;
mod package_change;
mod prepare_project;
mod publication;
mod stage_build_edit;

pub use accepted_compilation::{
    AdmittedProjectCompilation, ProjectCompilationAdmissionRequest, ProjectCompilationCompletion,
    ProjectCompilationError, ProjectCompilationOutcome, admit_project_compilation,
    admit_project_products,
};
pub use inspect_packages::{
    PackageInspectionError, PackageInspectionOptions, PackageInspectionOutcome,
    PackageInspectionSources, PackageInspectionTransaction, PreparedPackageInspection,
};
pub(crate) use inspect_source::PackageSourceRequest;
pub use inspect_source::{SourceAdapter, inspect_package_source_locator};
pub(crate) use package_change::{
    PackageChangeError, PackageChangeReview, review_package_change_reusing,
};
pub(crate) use prepare_project::LOCAL_PROJECT_CONTEXT;
pub use prepare_project::{
    LocalProjectPreparation, PrepareLocalProjectError, PreparedLocalProject,
    begin_local_project_preparation,
};
pub(crate) use publication::{
    PackageFileTransaction, PackagePublicationError, PackagePublicationLimits,
    publish_reviewed_package_change,
};
pub(crate) use stage_build_edit::stage_build_dependency_edit;
