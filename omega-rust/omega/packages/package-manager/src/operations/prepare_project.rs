/*
    We prepare one local `build.omg` project for package-aware compilation. The
    source closure is shared across requested product targets: an accepted
    lock's target sections pin the same immutable source subject, and an
    unlocked graph resolves the same way for any product target. What stays
    per-target is only the accepted policy section that arms each target's own
    restricted-request checkpoint.

    Beginning local project preparation recovers transactions and selects
    accepted target policy before acquiring an anonymous source, without
    parsing `Build` declarations, invoking a compiler, or granting restricted
    execution. A deleted declaration during a pending transaction is a recovery
    conflict, not permission to bypass package preparation as standalone. A
    single-target request rejects preparation immediately when its accepted
    section is missing or unreadable; multi-target requests retain the error on
    that target's row so sibling targets still compile. Completing source
    resolution verifies that neither `build.omg` nor `omega.lock` changed
    concurrently and routes the authored entry through invocation-owned source
    inputs without live-read permission or persistent snapshot copies.
*/

/*
    Package-owned project preparation and lock-baseline verification belong to
    the package manager's operations boundary (`operations/prepare_project.rs`).
*/

use super::{PackageFileTransaction, PackagePublicationError, PackagePublicationLimits};
use crate::lock::{PackageLock, PackageLockRecoveryLimits, PackageLockTarget};
use crate::resolution::graph::{
    CanonicalRootSourceRequest, GitDependencyPins, GitResolutionOptions, PackageRootSourceRequest,
    ResolvedPackageSourceClosure,
};
use crate::resolution::source::{AcquiredBuildSource, BuildSourceAcquisitions};
use package_compilation::PackageCompilationInputError;
use package_source::PrimaryGitChoices;
use package_source::git::resolution::GitExactRevisionAcquisition;
use package_source::{
    ExternalSourceContext, LocalSourceLimits, SourceResolveError, SourceResolver,
};
use std::fmt;
use std::path::{Path, PathBuf};
use target::TargetProfile;

#[cfg(test)]
mod tests;

pub const LOCAL_PROJECT_CONTEXT: &[u8] = b"omega-local-project-v1";

pub struct PreparedLocalProject {
    entry_path: PathBuf,
    source_closure: ResolvedPackageSourceClosure,
    targets: Vec<PreparedProjectTarget>,
}

pub struct LocalProjectPreparation {
    transaction: Option<PackageFileTransaction>,
    baseline: Option<(Vec<u8>, Option<Vec<u8>>)>,
    accepted: Option<PackageLock>,
    targets: Vec<PreparedProjectTarget>,
    canonical_root: PathBuf,
    entry_relative: PathBuf,
    authored_build_present: bool, // Preflight source observation, independent of acceptance state.
    storage: SourceResolver,
    offline: bool,
}

pub(super) struct PreparedProjectTarget {
    pub(super) profile: TargetProfile,
    pub(super) accepted: Result<Option<PackageLockTarget>, PrepareLocalProjectError>, // `Ok(None)` when unlocked; `Err` when this target's accepted section is missing or unreadable
}

impl PreparedLocalProject {
    pub fn entry_path(&self) -> &Path {
        &self.entry_path
    }

    pub fn root_package(&self) -> semantic_vocabulary::PackageKeyIdentity {
        self.source_closure.graph().root().identity()
    }

    pub fn package_at_local_root(
        &self,
        canonical_root: &Path,
    ) -> Option<semantic_vocabulary::PackageKeyIdentity> {
        self.source_closure
            .graph()
            .packages()
            .iter()
            .find_map(|package| {
                let key = package.source().key();
                match key.source_lineage() {
                    package_source::SourceLineage::ExternalLocal(local)
                        if local.canonical_absolute_path() == canonical_root =>
                    {
                        Some(key.identity())
                    }
                    _ => None,
                }
            })
    }

    pub(super) fn into_target_review_parts(
        self,
    ) -> (
        PathBuf,
        ResolvedPackageSourceClosure,
        Vec<PreparedProjectTarget>,
    ) {
        (self.entry_path, self.source_closure, self.targets)
    }
}

#[derive(Debug)]
pub enum PrepareLocalProjectError {
    Acquisition(crate::resolution::source::BuildSourceAcquisitionError),
    SourceJoin(String),
    NestedBuildActivationRequired {
        package: crate::declarations::PackageKey,
    },
    EntryOutsideProject {
        entry: PathBuf,
        project_root: PathBuf,
    },
    Storage(SourceResolveError),
    MissingRootCustody,
    CompilerInputs(Vec<PackageCompilationInputError>),
    Publication(PackagePublicationError),
    Locked(String),
}

impl fmt::Display for PrepareLocalProjectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Acquisition(error) => error.fmt(formatter),
            Self::SourceJoin(reason) => write!(
                formatter,
                "cannot complete project source preparation: {reason}"
            ),
            Self::NestedBuildActivationRequired { package } => write!(
                formatter,
                "package `{}` requires nested build activation; this direct compiler route does not execute dependency builds (use ordinary --check or compilation)",
                package.name().as_str()
            ),
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

pub fn begin_local_project_preparation(
    entry_path: &Path,
    targets: &[TargetProfile],
    offline: bool,
) -> Result<Option<LocalProjectPreparation>, PrepareLocalProjectError> {
    begin_local_project_preparation_in_storage(entry_path, targets, offline, |root| {
        SourceResolver::for_current_user(PrimaryGitChoices {
            excluded_controlled_roots: std::slice::from_ref(&root.to_path_buf()),
            ..PrimaryGitChoices::default()
        })
    })
}

fn begin_local_project_preparation_in_storage(
    entry_path: &Path,
    targets: &[TargetProfile],
    offline: bool,
    open_storage: impl FnOnce(&Path) -> Result<SourceResolver, SourceResolveError>,
) -> Result<Option<LocalProjectPreparation>, PrepareLocalProjectError> {
    if targets.is_empty() {
        return Err(PrepareLocalProjectError::Locked(
            "local project preparation needs at least one requested target".to_owned(),
        ));
    }
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
    let lock_present = match std::fs::symlink_metadata(project_root.join("omega.lock")) {
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(PrepareLocalProjectError::Locked(error.to_string())),
    };
    let authored_build_present = project_root.join("build.omg").is_file();
    if !authored_build_present && !lock_present {
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
    let prepared_targets = targets
        .iter()
        .map(|profile| {
            let section = match accepted.as_ref() {
                Some(lock) => lock.target(*profile).cloned().map(Some).ok_or_else(|| {
                    PrepareLocalProjectError::Locked(format!(
                        "no accepted section for exact target {}",
                        profile.target_name(),
                    ))
                }),
                None => Ok(None),
            };
            PreparedProjectTarget {
                profile: *profile,
                accepted: section,
            }
        })
        .collect::<Vec<_>>();
    let mut prepared_targets = prepared_targets;
    if let [single] = prepared_targets.as_mut_slice()
        && single.accepted.is_err()
    {
        return Err(std::mem::replace(&mut single.accepted, Ok(None)).unwrap_err());
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
    let storage =
        open_storage(&canonical_project_root).map_err(PrepareLocalProjectError::Storage)?;
    Ok(Some(LocalProjectPreparation {
        transaction,
        baseline,
        accepted,
        targets: prepared_targets,
        canonical_root: canonical_project_root,
        entry_relative,
        authored_build_present,
        storage,
        offline,
    }))
}

impl LocalProjectPreparation {
    pub fn entry_path(&self) -> PathBuf {
        self.canonical_root.join(&self.entry_relative)
    }

    pub fn acquire_root(&self) -> Result<AcquiredBuildSource, PrepareLocalProjectError> {
        self.acquire_selected_root(&package_source::SourceCaptureRequest::entire_tree())
    }

    pub fn acquire_selected_root(
        &self,
        selected: &package_source::SourceCaptureRequest,
    ) -> Result<AcquiredBuildSource, PrepareLocalProjectError> {
        let acquired = AcquiredBuildSource::capture_selected_local(
            &self.canonical_root,
            LocalSourceLimits::default(),
            ExternalSourceContext::derive(LOCAL_PROJECT_CONTEXT),
            selected,
        )
        .map_err(PrepareLocalProjectError::Acquisition)?;
        acquired
            .require_compilation_source(&self.entry_relative)
            .map_err(PrepareLocalProjectError::Acquisition)?;
        if self.authored_build_present {
            acquired
                .require_compilation_source(Path::new("build.omg"))
                .map_err(PrepareLocalProjectError::Acquisition)?;
        }
        Ok(acquired)
    }

    pub fn source_acquisitions(
        &self,
    ) -> Result<BuildSourceAcquisitions<'_>, PrepareLocalProjectError> {
        let source = self
            .accepted
            .as_ref()
            .and_then(|lock| lock.targets().first())
            .map(|target| target.source());
        if let Some(source) = source {
            let matches_root = matches!(source.root().request(), CanonicalRootSourceRequest::ExternalLocal { requested_root, source_context }
                if requested_root == self.canonical_root.as_os_str().as_encoded_bytes()
                    && source_context == &ExternalSourceContext::derive(LOCAL_PROJECT_CONTEXT));
            if !matches_root {
                return Err(PrepareLocalProjectError::Locked(
                    "accepted Git pins belong to another root request".into(),
                ));
            }
        }
        let acquisition = if self.offline {
            GitExactRevisionAcquisition::Offline
        } else {
            GitExactRevisionAcquisition::AllowFetch
        };
        let pins = source
            .map(|source| GitDependencyPins::new(source, &[], acquisition))
            .transpose()
            .map_err(|error| PrepareLocalProjectError::Locked(error.to_string()))?;
        Ok(crate::resolution::source::BuildSourceAcquisitions::new(
            &self.storage,
            GitResolutionOptions {
                pins,
                offline: self.offline,
            },
            PackageRootSourceRequest::ExternalLocal {
                requested_root: self.canonical_root.clone(),
                source_context: ExternalSourceContext::derive(LOCAL_PROJECT_CONTEXT),
            },
        ))
    }

    pub fn complete_source_resolution(
        self,
        resolved: crate::resolution::source::ResolvedBuildSources,
    ) -> Result<PreparedLocalProject, PrepareLocalProjectError> {
        self.complete(resolved.into_source_closure())
    }

    fn complete(
        self,
        closure: ResolvedPackageSourceClosure,
    ) -> Result<PreparedLocalProject, PrepareLocalProjectError> {
        let expected = PackageRootSourceRequest::ExternalLocal {
            requested_root: self.canonical_root.clone(),
            source_context: ExternalSourceContext::derive(LOCAL_PROJECT_CONTEXT),
        };
        if closure.source_requests().root().request() != &expected {
            return Err(PrepareLocalProjectError::SourceJoin(
                "resolved graph belongs to another project acquisition".into(),
            ));
        }
        if let (Some(transaction), Some(baseline)) = (&self.transaction, &self.baseline) {
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
        let source_root = closure
            .source_root(closure.graph().root())
            .ok_or(PrepareLocalProjectError::MissingRootCustody)?;
        let prepared_entry = source_root.join(self.entry_relative);
        Ok(PreparedLocalProject {
            entry_path: prepared_entry,
            source_closure: closure,
            targets: self.targets,
        })
    }
}
