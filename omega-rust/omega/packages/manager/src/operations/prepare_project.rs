//! Prepare one local `build.omg` project for package-aware compilation.

use super::{PackageFileTransaction, PackagePublicationError, PackagePublicationLimits};
use crate::resolution::graph::{
    PackageSourceClosureLimits, ResolveExternalLocalPackageClosureError,
    ResolvedPackageSourceClosure, resolve_external_local_project_closure_with_storage,
};
use crate::resolution::package_compilation_inputs;
use package_compilation::{PackageCompilationInputError, PackageCompilationInputs};
use package_source::{
    ExternalSourceContext, LocalSourceLimits, SourceResolveError, SourceResolverStorage,
};
use std::fmt;
use std::path::{Path, PathBuf};

const LOCAL_PROJECT_CONTEXT: &[u8] = b"omega-local-project-v1";

/// The compiler entry and exact package graph prepared from one local project.
pub struct PreparedLocalProject {
    entry_path: PathBuf,
    package_inputs: PackageCompilationInputs,
    source_closure: ResolvedPackageSourceClosure,
}

impl PreparedLocalProject {
    #[must_use]
    pub fn into_parts(self) -> (PathBuf, PackageCompilationInputs) {
        (self.entry_path, self.package_inputs)
    }

    pub(super) fn into_review_parts(self) -> (PathBuf, ResolvedPackageSourceClosure) {
        (self.entry_path, self.source_closure)
    }
}

#[derive(Debug)]
pub enum PrepareLocalProjectError {
    EntryOutsideProject {
        entry: PathBuf,
        project_root: PathBuf,
    },
    Storage(SourceResolveError),
    Closure(ResolveExternalLocalPackageClosureError),
    MissingRootCustody,
    CompilerInputs(Vec<PackageCompilationInputError>),
    Publication(PackagePublicationError),
}

impl fmt::Display for PrepareLocalProjectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Publication(error) => error.fmt(formatter),
            Self::EntryOutsideProject {
                entry,
                project_root,
            } => write!(
                formatter,
                "entry source {} is outside its selected project root {}",
                entry.display(),
                project_root.display(),
            ),
            Self::Storage(error) => {
                write!(
                    formatter,
                    "cannot open private source resolver storage: {error}"
                )
            }
            Self::Closure(error) => {
                write!(
                    formatter,
                    "cannot resolve declared package closure: {error}"
                )
            }
            Self::MissingRootCustody => {
                formatter.write_str("resolved package closure lost its root source custody")
            }
            Self::CompilerInputs(errors) => {
                write!(
                    formatter,
                    "cannot construct compiler package graph: {errors:?}"
                )
            }
        }
    }
}

impl std::error::Error for PrepareLocalProjectError {}

/// Prepare package-aware compiler inputs when `entry_path` belongs to a local
/// project with a sibling `build.omg`.
///
/// A standalone Omega source returns `Ok(None)`. The workflow owns storage,
/// source closure resolution, root relocation, and compiler input assembly so
/// the command-line binary does not reproduce package policy. Target choice is
/// deliberately later than this target-independent preparation.
pub fn prepare_local_project(
    entry_path: &Path,
) -> Result<Option<PreparedLocalProject>, PrepareLocalProjectError> {
    let project_root = entry_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let mut transaction =
        PackageFileTransaction::open_if_present(&project_root, PackagePublicationLimits::default())
            .map_err(PrepareLocalProjectError::Publication)?;
    if let Some(transaction) = &mut transaction {
        transaction
            .recover()
            .map_err(PrepareLocalProjectError::Publication)?;
    }
    // A deleted declaration during a pending transaction is a recovery
    // conflict, not permission to bypass package preparation as standalone.
    if !project_root.join("build.omg").is_file() {
        return Ok(None);
    }
    let entry_relative = if entry_path.is_relative() && project_root == Path::new(".") {
        entry_path.to_path_buf()
    } else {
        entry_path
            .strip_prefix(&project_root)
            .map(Path::to_path_buf)
            .map_err(|_| PrepareLocalProjectError::EntryOutsideProject {
                entry: entry_path.to_path_buf(),
                project_root: project_root.clone(),
            })?
    };
    let canonical_project_root = project_root.canonicalize().map_err(|error| {
        PrepareLocalProjectError::Storage(SourceResolveError::Io {
            path: project_root.clone(),
            message: error.to_string(),
        })
    })?;
    let storage = SourceResolverStorage::for_current_user_excluding_primary_git_roots(
        std::slice::from_ref(&canonical_project_root),
    )
    .map_err(PrepareLocalProjectError::Storage)?;
    let closure = resolve_external_local_project_closure_with_storage(
        &project_root,
        ExternalSourceContext::derive(LOCAL_PROJECT_CONTEXT),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .map_err(PrepareLocalProjectError::Closure)?;
    let root_snapshot = closure
        .source_root(closure.graph().root())
        .ok_or(PrepareLocalProjectError::MissingRootCustody)?;
    let prepared_entry = root_snapshot.join(entry_relative);
    let package_inputs =
        package_compilation_inputs(&closure).map_err(PrepareLocalProjectError::CompilerInputs)?;
    Ok(Some(PreparedLocalProject {
        entry_path: prepared_entry,
        package_inputs,
        source_closure: closure,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time follows Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "omega-package-workflow-{name}-{}-{stamp}",
            std::process::id()
        ))
    }

    #[test]
    fn standalone_source_does_not_enter_package_resolution() {
        let root = temporary_root("standalone");
        std::fs::create_dir_all(&root).expect("create standalone source root");
        let entry = root.join("main.omg");
        std::fs::write(&entry, "machine main() {}\n").expect("write standalone source");

        let prepared = prepare_local_project(&entry).expect("inspect standalone source");

        assert!(prepared.is_none());
        std::fs::remove_dir_all(root).expect("remove standalone source root");
    }
}
