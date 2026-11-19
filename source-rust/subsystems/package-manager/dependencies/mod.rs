/*
    We turn checked `build.omg` declarations into an exact immutable package
    closure and independently validated compiler inputs. `source` binds declared
    local, workspace, and Git locations to source custody, `graph` follows those
    bindings, rejects conflicts, and gives the complete closure a canonical
    review subject, and `compiler_input` revalidates that custody at the
    compiler handoff.
*/

/*
    Package source and graph resolution belong to the package manager's
    resolution boundary (`resolution/mod.rs`).
*/

mod compiler_input;
pub mod graph;
pub mod source;

pub(crate) use compiler_input::PackageCompilationScope;

mod accepted_compilation_identity;
mod reviewed_host_packages;
pub mod declarations;
mod prepare_project;
mod stage_build_edit;

pub(crate) use accepted_compilation_identity::compiler_input_identity;
pub use reviewed_host_packages::ReviewedHostPackages;
pub(crate) use prepare_project::LOCAL_PROJECT_CONTEXT;
pub use prepare_project::{
    AcceptedDeployment, LocalProjectPreparation, PrepareLocalProjectError, PreparedLocalProject,
    begin_local_project_preparation,
};
pub(crate) use stage_build_edit::stage_build_dependency_edit;
