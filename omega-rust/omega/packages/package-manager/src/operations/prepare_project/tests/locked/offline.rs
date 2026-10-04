use super::super::super::PreparedLocalProject;
// Only the Unix Git-transport leg drives a real repository through paths.
#[cfg(unix)]
use super::super::super::Path;
#[cfg(unix)]
use super::super::PathBuf;
#[cfg(unix)]
use super::LocalSourceLimits;
use super::{
    LocalProjectPreparationOptions, PrepareLocalProjectError, Project, fs, preparation_target,
    prepare_local_project_in_storage,
};
fn prepare(project: &Project) -> Result<Option<PreparedLocalProject>, PrepareLocalProjectError> {
    prepare_local_project_in_storage(
        &project.root().join("main.omg"),
        LocalProjectPreparationOptions {
            target: preparation_target(),
            offline: true,
        },
        |_| Ok(project.storage()),
    )
}
