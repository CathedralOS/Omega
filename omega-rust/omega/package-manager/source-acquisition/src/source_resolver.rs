//! Source-resolution session: retain private storage and select host Git once.

use crate::SourceResolveError;
use crate::custody::tree::CacheCustodyKind;
use crate::git::executable::selection::{PrimaryGitSelection, resolver_package_controlled_roots};
use crate::storage::{RetainedStorageLane, RetainedStorageRoot, current_user_cache_base};
use std::path::{Path, PathBuf};

const GIT_SOURCES: &str = "git-sources";
const WORKSPACE_MEMBERS: &str = "workspace-members";
const EXTERNAL_LOCAL_SOURCES: &str = "external-local-sources";

/// One source-resolution session with retained directory custody and host Git selection.
///
/// Git quarantine, local snapshots, publication stages, and locks remain descendants
/// of the retained root. Git selection belongs to this session, not its directories;
/// local-only resolution remains available when no host Git executable was found.
#[derive(Debug)]
pub struct SourceResolver {
    root: RetainedStorageRoot,
    git_sources: RetainedStorageLane,
    workspace_members: RetainedStorageLane,
    external_local_sources: RetainedStorageLane,
    primary_git: Option<PrimaryGitSelection>,
}

/// Operator choices that narrow how the resolver selects its primary Git executable.
///
/// The default lets the resolver select automatically from the host; an explicit
/// executable or excluded controlled roots come from operator flags.
#[derive(Clone, Copy, Debug, Default)]
pub struct PrimaryGitChoices<'a> {
    /// One explicit operator-selected Git executable.
    pub explicit_git: Option<&'a Path>,
    /// Operator-known controlled roots excluded from automatic selection.
    pub excluded_controlled_roots: &'a [PathBuf],
}

impl SourceResolver {
    /// Open or create the current user's production source resolver.
    ///
    /// There is deliberately no temporary-directory fallback. If the host has
    /// no usable per-user cache location, source resolution fails closed.
    pub fn for_current_user(choices: PrimaryGitChoices<'_>) -> Result<Self, SourceResolveError> {
        let base = current_user_cache_base()?;
        Self::create_beneath(&base, choices)
    }

    /// Open an isolated resolver tree beneath a caller-custodied base.
    ///
    /// This is the explicit hardened-mode constructor for CI, bootstrap, and
    /// other infrastructure that supplies its own private storage boundary.
    /// Ordinary commands must use [`Self::for_current_user`].
    pub fn for_hardened_base(
        base: impl AsRef<Path>,
        choices: PrimaryGitChoices<'_>,
    ) -> Result<Self, SourceResolveError> {
        Self::create_beneath(base.as_ref(), choices)
    }

    #[cfg(test)]
    pub(crate) fn root(&self) -> &Path {
        self.root.path()
    }

    pub fn git_sources(&self) -> &RetainedStorageLane {
        &self.git_sources
    }

    pub fn workspace_members(&self) -> &RetainedStorageLane {
        &self.workspace_members
    }

    pub fn external_local_sources(&self) -> &RetainedStorageLane {
        &self.external_local_sources
    }

    /// The host Git selected when this session opened.
    ///
    /// Git lane entrances take this selection explicitly; directory custody does
    /// not choose an executable or prevent callers supplying another selection.
    pub fn primary_git(&self) -> Result<&PrimaryGitSelection, SourceResolveError> {
        self.primary_git
            .as_ref()
            .ok_or(SourceResolveError::GitExecutableUnavailable)
    }

    pub fn verify_path_identity(&self) -> Result<(), SourceResolveError> {
        self.root.verify_path_identity()?;
        for lane in [
            &self.git_sources,
            &self.workspace_members,
            &self.external_local_sources,
        ] {
            lane.verify_path_identity()?;
        }
        Ok(())
    }

    fn create_beneath(
        base: &Path,
        choices: PrimaryGitChoices<'_>,
    ) -> Result<Self, SourceResolveError> {
        let PrimaryGitChoices {
            explicit_git: explicit_primary_git,
            excluded_controlled_roots: excluded_primary_git_roots,
        } = choices;
        let root = RetainedStorageRoot::create_beneath(base)?;
        let mut excluded_roots = resolver_package_controlled_roots(&[root.path()])?;
        excluded_roots
            .try_reserve_exact(excluded_primary_git_roots.len())
            .map_err(|_| SourceResolveError::PrivateStorageUnavailable {
                message: "primary Git exclusion set exceeds host capacity".to_owned(),
            })?;
        for excluded in excluded_primary_git_roots {
            if !excluded.is_absolute() {
                return Err(SourceResolveError::PrivateStorageUnavailable {
                    message: format!(
                        "primary Git exclusion root `{}` is not absolute",
                        excluded.display()
                    ),
                });
            }
            excluded_roots.push(excluded.clone());
        }
        let primary_git =
            PrimaryGitSelection::capture_optional(explicit_primary_git, &excluded_roots)?;

        let git_sources = root.create_lane(GIT_SOURCES, CacheCustodyKind::Git)?;
        let workspace_members =
            root.create_lane(WORKSPACE_MEMBERS, CacheCustodyKind::LocalSnapshot)?;
        let external_local_sources =
            root.create_lane(EXTERNAL_LOCAL_SOURCES, CacheCustodyKind::LocalSnapshot)?;
        let resolver = Self {
            root,
            git_sources,
            workspace_members,
            external_local_sources,
            primary_git,
        };
        resolver.verify_path_identity()?;
        Ok(resolver)
    }
}

#[cfg(test)]
mod tests {
    use super::PrimaryGitChoices;
    use super::{PathBuf, SourceResolver};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn isolated_base(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "omega-private-resolver-{name}-{}-{stamp}",
            std::process::id()
        ))
    }

    #[test]
    fn production_constructor_creates_one_retained_private_tree() {
        let base = isolated_base("storage");
        std::fs::create_dir_all(&base).expect("create isolated cache base");
        let storage = SourceResolver::create_beneath(&base, PrimaryGitChoices::default())
            .expect("production private-root constructor");
        assert!(storage.root().starts_with(base.canonicalize().unwrap()));
        storage
            .verify_path_identity()
            .expect("retained root identity");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for component in ["CathedralOS", "Omega", "source", "v1"] {
                let path = storage
                    .root()
                    .ancestors()
                    .find(|path| path.file_name().is_some_and(|name| name == component))
                    .expect("managed component");
                assert_eq!(
                    std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                    0o700,
                );
            }
            for lane in [
                storage.git_sources(),
                storage.workspace_members(),
                storage.external_local_sources(),
            ] {
                assert_eq!(
                    std::fs::metadata(lane.path()).unwrap().permissions().mode() & 0o777,
                    0o700,
                );
            }
        }

        drop(storage);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    #[cfg_attr(
        windows,
        ignore = "Windows prevents replacement while the retained storage-root handle is open"
    )]
    fn retained_private_root_rejects_path_replacement() {
        let base = isolated_base("replacement");
        std::fs::create_dir_all(&base).expect("create isolated cache base");
        let storage = SourceResolver::create_beneath(&base, PrimaryGitChoices::default())
            .expect("production private-root constructor");
        let retained = storage.root().with_extension("retained");
        std::fs::rename(storage.root(), &retained).expect("move retained root");
        std::fs::create_dir(storage.root()).expect("replace root pathname");
        let error = storage
            .verify_path_identity()
            .expect_err("replacement must not satisfy retained storage custody");
        assert!(error.to_string().contains("no longer identifies"));

        let _ = std::fs::remove_dir_all(storage.root());
        drop(storage);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    #[cfg_attr(
        windows,
        ignore = "Windows prevents replacement while the retained storage-lane handle is open"
    )]
    fn retained_private_lane_rejects_path_replacement() {
        let base = isolated_base("lane-replacement");
        std::fs::create_dir_all(&base).expect("create isolated cache base");
        let storage = SourceResolver::create_beneath(&base, PrimaryGitChoices::default())
            .expect("production private-root constructor");
        let lane = storage.git_sources().path().to_path_buf();
        let retained = lane.with_extension("retained");
        std::fs::rename(&lane, &retained).expect("move retained lane");
        std::fs::create_dir(&lane).expect("replace lane pathname");
        let error = storage
            .verify_path_identity()
            .expect_err("replacement must not satisfy retained lane custody");
        assert!(error.to_string().contains("no longer identifies"));

        drop(storage);
        let _ = std::fs::remove_dir_all(base);
    }
}
