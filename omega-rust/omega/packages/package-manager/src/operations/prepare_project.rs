//! Prepare one local `build.omg` project for package-aware compilation.

use super::{PackageFileTransaction, PackagePublicationError, PackagePublicationLimits};
use crate::lock::{PackageLock, PackageLockRecoveryLimits, PackageLockTarget};
use crate::resolution::graph::{
    CanonicalSourceClosureSubjectLimits, GitResolutionOptions, PackageRootSourceRequest,
    PackageSourceClosureLimits, ResolveExternalLocalPackageClosureError,
    ResolvedPackageSourceClosure, resolve_external_local_project_closure,
    resolve_locked_local_project_closure,
};
use crate::resolution::package_compilation_inputs;
use package_compilation::{PackageCompilationInputError, PackageCompilationInputs};
use package_source::PrimaryGitChoices;
use package_source::git::resolution::GitExactRevisionAcquisition;
use package_source::{
    ExternalSourceContext, LocalSourceLimits, SourceResolveError, SourceResolver,
};
use std::fmt;
use std::path::{Path, PathBuf};
use target::TargetProfile;

pub(crate) const LOCAL_PROJECT_CONTEXT: &[u8] = b"omega-local-project-v1";

/// Invocation-local source acquisition choices; never stored as lock policy.
#[derive(Debug, Clone, Copy)]
pub struct LocalProjectPreparationOptions {
    pub target: TargetProfile,
    pub offline: bool,
}

/// The compiler entry and exact package graph prepared from one local project,
/// covering every requested target at once.
///
/// The source closure is shared: an accepted lock's target sections pin the
/// same immutable source subject, and an unlocked graph resolves the same way
/// for any product target. What stays per-target is only the accepted policy
/// section that arms each target's own restricted-request checkpoint.
pub struct PreparedLocalProject {
    entry_path: PathBuf,
    package_inputs: PackageCompilationInputs,
    source_closure: ResolvedPackageSourceClosure,
    targets: Vec<PreparedProjectTarget>,
}

/// One requested target's view of the prepared project: its exact profile and
/// the accepted lock section prepared for it, or the preparation error that
/// target alone must report.
pub(super) struct PreparedProjectTarget {
    pub(super) profile: TargetProfile,
    /// `Ok(None)` names an unlocked project; `Err` is this target's missing or
    /// unreadable accepted section — sibling targets still compile.
    pub(super) accepted: Result<Option<PackageLockTarget>, PrepareLocalProjectError>,
}

impl PreparedLocalProject {
    /// Extract acquisition-only inputs for a direct compiler consumer. These
    /// inputs deliberately omit dependencies' private build edges; a nested
    /// activation must instead use the review-aware check/production route,
    /// which executes prerequisites and carries their generated source.
    pub fn try_into_parts(
        self,
    ) -> Result<(PathBuf, PackageCompilationInputs), PrepareLocalProjectError> {
        let graph = self.source_closure.graph();
        if let Some(package) = graph.packages().iter().find(|package| {
            package.source().key() != graph.root()
                && package.dependencies().iter().any(|dependency| {
                    dependency.purpose() == crate::declarations::DependencyPurpose::Build
                })
        }) {
            return Err(PrepareLocalProjectError::NestedBuildActivationRequired {
                package: package.source().key().clone(),
            });
        }
        Ok((self.entry_path, self.package_inputs))
    }

    /// Split for one exact target's review route: the shared entry and source
    /// closure plus that target's accepted policy section. A prepared project
    /// that does not cover `profile`, or whose own section failed, rejects
    /// here rather than silently substituting another target's consent.
    pub(super) fn into_review_parts(
        self,
        profile: TargetProfile,
    ) -> Result<
        (
            PathBuf,
            ResolvedPackageSourceClosure,
            Option<PackageLockTarget>,
        ),
        PrepareLocalProjectError,
    > {
        let PreparedLocalProject {
            entry_path,
            source_closure,
            targets,
            ..
        } = self;
        let target = targets
            .into_iter()
            .find(|target| target.profile == profile)
            .ok_or_else(|| {
                PrepareLocalProjectError::Locked(format!(
                    "prepared project covers no section for exact target {}",
                    profile.target_name(),
                ))
            })?;
        Ok((entry_path, source_closure, target.accepted?))
    }

    /// Split for a multi-target route: every requested target's section beside
    /// the shared entry path and source closure.
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
    NestedBuildActivationRequired {
        package: crate::declarations::PackageKey,
    },
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

/// Offline preparation permits normal local root edits and cached accepted Git
/// pins. Unlocked local graphs work, but any unrecorded Git request rejects.
/// Prepare package-aware compiler inputs when `entry_path` belongs to a local
/// project with a sibling `build.omg`; `options` selects the exact target
/// (accepted evidence is chosen for it before any source is acquired) and the
/// offline policy.
///
/// A standalone Omega source returns `Ok(None)`. The workflow owns storage,
/// source closure resolution, root relocation, and compiler input assembly so
/// the command-line binary does not reproduce package policy. Local project
/// source stays editable; accepted dependency content and requests remain
/// pinned until an explicit package update.
pub fn prepare_local_project(
    entry_path: &Path,
    options: LocalProjectPreparationOptions,
) -> Result<Option<PreparedLocalProject>, PrepareLocalProjectError> {
    prepare_local_project_in_storage(entry_path, options, |root| {
        SourceResolver::for_current_user(PrimaryGitChoices {
            excluded_controlled_roots: std::slice::from_ref(&root.to_path_buf()),
            ..PrimaryGitChoices::default()
        })
    })
}

fn prepare_local_project_in_storage(
    entry_path: &Path,
    options: LocalProjectPreparationOptions,
    open_storage: impl FnOnce(&Path) -> Result<SourceResolver, SourceResolveError>,
) -> Result<Option<PreparedLocalProject>, PrepareLocalProjectError> {
    prepare_local_project_for_targets_in_storage(
        entry_path,
        &[options.target],
        options.offline,
        open_storage,
    )
}

/// The same preparation for several requested targets at once: source
/// resolution, transaction recovery, and custody checks run once, and each
/// target keeps only its own accepted lock section. A target with no accepted
/// section carries its error in the returned project so sibling targets still
/// compile. `targets` must not be empty.
pub fn prepare_local_project_for_targets(
    entry_path: &Path,
    targets: &[TargetProfile],
    offline: bool,
) -> Result<Option<PreparedLocalProject>, PrepareLocalProjectError> {
    prepare_local_project_for_targets_in_storage(entry_path, targets, offline, |root| {
        SourceResolver::for_current_user(PrimaryGitChoices {
            excluded_controlled_roots: std::slice::from_ref(&root.to_path_buf()),
            ..PrimaryGitChoices::default()
        })
    })
}

fn prepare_local_project_for_targets_in_storage(
    entry_path: &Path,
    targets: &[TargetProfile],
    offline: bool,
    open_storage: impl FnOnce(&Path) -> Result<SourceResolver, SourceResolveError>,
) -> Result<Option<PreparedLocalProject>, PrepareLocalProjectError> {
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
    // A lock's target sections pin the same immutable source subject, so the
    // closure resolves once; each target then keeps only its own section.
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
    // A single-target request keeps the historical failure surface: its one
    // missing section rejects preparation itself, before storage opens or a
    // missing local source acquires, rather than landing on the target row.
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
    let closure = if let Some(accepted) = accepted.as_ref() {
        resolve_locked_local_project_closure(
            accepted
                .targets()
                .first()
                .expect("a recovered lock validates a nonempty target roster")
                .source(),
            &PackageRootSourceRequest::ExternalLocal {
                requested_root: canonical_project_root.clone(),
                source_context: ExternalSourceContext::derive(LOCAL_PROJECT_CONTEXT),
            },
            if offline {
                GitExactRevisionAcquisition::Offline
            } else {
                GitExactRevisionAcquisition::AllowFetch
            },
            &storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .map_err(|error| PrepareLocalProjectError::Locked(error.to_string()))?
    } else {
        resolve_external_local_project_closure(
            &canonical_project_root,
            ExternalSourceContext::derive(LOCAL_PROJECT_CONTEXT),
            &storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
            GitResolutionOptions {
                pins: None,
                offline,
            },
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
        targets: prepared_targets,
    }))
}
