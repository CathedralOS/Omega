//! Prepare one local `build.omg` project for package-aware compilation.

use super::{PackageFileTransaction, PackagePublicationError, PackagePublicationLimits};
use crate::lock::{PackageLock, PackageLockRecoveryLimits};
use crate::resolution::graph::{
    CanonicalSourceClosureSubjectLimits, PackageRootSourceRequest, PackageSourceClosureLimits,
    ResolveExternalLocalPackageClosureError, ResolvedPackageSourceClosure,
    resolve_external_local_project_closure_with_storage,
    resolve_locked_local_project_closure_with_storage,
};
use crate::resolution::package_compilation_inputs;
use package_compilation::{PackageCompilationInputError, PackageCompilationInputs};
use package_source::git::resolution::GitExactRevisionAcquisition;
use package_source::{
    ExternalSourceContext, LocalSourceLimits, SourceResolveError, SourceResolverStorage,
};
use std::fmt;
use std::path::{Path, PathBuf};
use target::TargetProfile;

pub(super) const LOCAL_PROJECT_CONTEXT: &[u8] = b"omega-local-project-v1";

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
    Locked(String),
}

impl fmt::Display for PrepareLocalProjectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Locked(detail) => write!(
                formatter,
                "cannot prepare accepted omega.lock: {detail}; run omega update for fresh review (restore a compatible lock or explicitly remove an unsupported lock first); no selector was refreshed"
            ),
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
/// the command-line binary does not reproduce package policy. This convenience
/// wrapper selects the host target; cross-target callers use the exact-target
/// entrance below.
pub fn prepare_local_project(
    entry_path: &Path,
) -> Result<Option<PreparedLocalProject>, PrepareLocalProjectError> {
    prepare_local_project_for_target(entry_path, TargetProfile::host())
}

/// Select accepted evidence for the exact target before acquiring any source.
/// Local project source stays editable; accepted dependency content and requests
/// remain pinned until an explicit package update.
pub fn prepare_local_project_for_target(
    entry_path: &Path,
    target: TargetProfile,
) -> Result<Option<PreparedLocalProject>, PrepareLocalProjectError> {
    prepare_with_storage(entry_path, target, |root| {
        SourceResolverStorage::for_current_user_excluding_primary_git_roots(std::slice::from_ref(
            &root.to_path_buf(),
        ))
    })
}

fn prepare_with_storage(
    entry_path: &Path,
    target: TargetProfile,
    open_storage: impl FnOnce(&Path) -> Result<SourceResolverStorage, SourceResolveError>,
) -> Result<Option<PreparedLocalProject>, PrepareLocalProjectError> {
    let project_root = entry_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let mut transaction =
        PackageFileTransaction::open_if_present(&project_root, PackagePublicationLimits::default())
            .map_err(PrepareLocalProjectError::Publication)?;
    if transaction.is_none() {
        match std::fs::symlink_metadata(project_root.join("omega.lock")) {
            Ok(_) => {
                transaction = Some(
                    PackageFileTransaction::open(
                        &project_root,
                        PackagePublicationLimits::default(),
                    )
                    .map_err(PrepareLocalProjectError::Publication)?,
                );
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(PrepareLocalProjectError::Locked(error.to_string())),
        }
    }
    if let Some(transaction) = &mut transaction {
        transaction
            .recover()
            .map_err(PrepareLocalProjectError::Publication)?;
    }
    // A deleted declaration during a pending transaction is a recovery
    // conflict, not permission to bypass package preparation as standalone.
    let lock_present = match std::fs::symlink_metadata(project_root.join("omega.lock")) {
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(PrepareLocalProjectError::Locked(error.to_string())),
    };
    if !project_root.join("build.omg").is_file() && !lock_present {
        return Ok(None);
    }
    let baseline = transaction
        .as_ref()
        .map(PackageFileTransaction::read_pair)
        .transpose()
        .map_err(PrepareLocalProjectError::Publication)?;
    let accepted = baseline
        .as_ref()
        .and_then(|(_, lock)| lock.as_deref())
        .map(|bytes| {
            let text = std::str::from_utf8(bytes)
                .map_err(|error| PrepareLocalProjectError::Locked(error.to_string()))?;
            PackageLock::recover_text(text, PackageLockRecoveryLimits::default())
                .map_err(|error| PrepareLocalProjectError::Locked(error.to_string()))
        })
        .transpose()?;
    let accepted_target = accepted
        .as_ref()
        .map(|lock| {
            lock.target(target).ok_or_else(|| {
                PrepareLocalProjectError::Locked(format!(
                    "no accepted section for exact target {}",
                    target.target_name(),
                ))
            })
        })
        .transpose()?;
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
    let storage =
        open_storage(&canonical_project_root).map_err(PrepareLocalProjectError::Storage)?;
    let closure = if let Some(accepted) = accepted_target {
        resolve_locked_local_project_closure_with_storage(
            accepted.source(),
            &PackageRootSourceRequest::ExternalLocal {
                requested_root: canonical_project_root.clone(),
                source_context: ExternalSourceContext::derive(LOCAL_PROJECT_CONTEXT),
            },
            GitExactRevisionAcquisition::AllowFetch,
            &storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .map_err(|error| PrepareLocalProjectError::Locked(error.to_string()))?
    } else {
        resolve_external_local_project_closure_with_storage(
            &canonical_project_root,
            ExternalSourceContext::derive(LOCAL_PROJECT_CONTEXT),
            &storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .map_err(PrepareLocalProjectError::Closure)?
    };
    if let (Some(transaction), Some(baseline)) = (&transaction, &baseline) {
        let current = transaction
            .read_pair()
            .map_err(PrepareLocalProjectError::Publication)?;
        if current.0 != baseline.0 {
            return Err(PrepareLocalProjectError::Publication(
                PackagePublicationError::ConcurrentEdit { file: "build.omg" },
            ));
        }
        if current.1 != baseline.1 {
            return Err(PrepareLocalProjectError::Publication(
                PackagePublicationError::ConcurrentEdit { file: "omega.lock" },
            ));
        }
    }
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
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    mod locked;

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn temporary_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time follows Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "omega-package-workflow-{name}-{}-{stamp}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed),
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
