//! Operations over a retained, custody-verified bare Git repository.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use cap_std::fs::{Dir as CapabilityDirectory, Metadata as CapabilityMetadata};
use omega_resolver_execution::ResolverExecutionPhase;

use crate::SourceResolveError;
use crate::custody::lock::verify_retained_cache_parent_path;
use crate::custody::platform::{
    verify_capability_cache_node_owner_and_mode, verify_macos_open_cache_extended_acl_custody,
};
use crate::custody::publication::{
    ProvisionalCacheDirectory, create_private_cache_directory, retain_private_cache_directory,
};
use crate::custody::tree::{
    CacheCustodyKind, git_cache_custody_byte_limit, read_bounded_cache_record_from_open_directory,
    verify_cache_custody_from_open_root,
};
use crate::git::cache::configuration::replace_canonical_git_control_file_from_open_repository;
use crate::git::commands::invocation::{run_git, run_git_bytes_stdout, run_git_stdout};
use crate::git::commands::reconciliation::reconcile_git_cache_operation_result;
use crate::git::executable::executor::GitExecutor;
use crate::git::request::GitExecutionTransport;
use crate::limits::{
    GIT_CACHE_METADATA, GIT_CACHE_REPOSITORY, GIT_CACHE_SNAPSHOTS, GIT_CONFIG_SHA1,
    GIT_CONFIG_SHA256, LocalSourceLimits,
};
use crate::tree::filesystem::io_error;

use super::custody::{
    open_retained_git_directory, reject_retained_git_path,
    verify_git_repository_tree_from_open_root, verify_retained_git_directory_identity,
};
use super::identity::git_cache_metadata;
use super::snapshots::RetainedGitSnapshots;
use crate::error::cache_invalid;

pub(crate) struct VerifiedGitRepository {
    pub(crate) entry_root: PathBuf,
    pub(crate) repository_path: PathBuf,
    pub(crate) entry_name: OsString,
    pub(crate) expected_metadata: Vec<u8>,
    pub(crate) cache_parent: CapabilityDirectory,
    pub(crate) entry: CapabilityDirectory,
    pub(crate) repository: CapabilityDirectory,
    pub(crate) objects: CapabilityDirectory,
    pub(crate) entry_identity: CapabilityMetadata,
    pub(crate) repository_identity: CapabilityMetadata,
    pub(crate) objects_identity: CapabilityMetadata,
}

impl VerifiedGitRepository {
    pub(crate) fn open(
        cache_parent: &CapabilityDirectory,
        entry_name: &OsStr,
        entry_root: &Path,
        url: &str,
        requested_rev: &str,
        execution_transport: GitExecutionTransport,
        limits: LocalSourceLimits,
    ) -> Result<Self, SourceResolveError> {
        let (entry, entry_identity) = open_retained_git_directory(
            cache_parent,
            entry_name,
            entry_root,
            "cache entry root is not a concrete directory",
        )?;
        let repository_path = entry_root.join(GIT_CACHE_REPOSITORY);
        let (repository, repository_identity) = open_retained_git_directory(
            &entry,
            OsStr::new(GIT_CACHE_REPOSITORY),
            &repository_path,
            "repository is not a concrete directory",
        )?;
        let objects_path = repository_path.join("objects");
        let (objects, objects_identity) = open_retained_git_directory(
            &repository,
            OsStr::new("objects"),
            &objects_path,
            "Git object directory is not a concrete directory",
        )?;
        let verified = Self {
            entry_root: entry_root.to_path_buf(),
            repository_path,
            entry_name: entry_name.to_os_string(),
            expected_metadata: git_cache_metadata(url, requested_rev, execution_transport),
            cache_parent: cache_parent
                .try_clone()
                .map_err(|error| io_error(entry_root, error))?,
            entry,
            repository,
            objects,
            entry_identity,
            repository_identity,
            objects_identity,
        };
        verified.verify_current(limits)?;
        Ok(verified)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.repository_path
    }

    pub(crate) fn verify_identity(&self) -> Result<(), SourceResolveError> {
        let cache_root = self.entry_root.parent().ok_or_else(|| {
            cache_invalid(&self.entry_root, "Git cache entry has no retained parent")
        })?;
        verify_retained_cache_parent_path(CacheCustodyKind::Git, cache_root, &self.cache_parent)?;
        verify_retained_git_directory_identity(
            &self.cache_parent,
            &self.entry_name,
            &self.entry,
            &self.entry_identity,
            &self.entry_root,
            "cache entry root no longer identifies the retained directory",
        )?;
        verify_retained_git_directory_identity(
            &self.entry,
            OsStr::new(GIT_CACHE_REPOSITORY),
            &self.repository,
            &self.repository_identity,
            &self.repository_path,
            "repository no longer identifies the retained directory",
        )?;
        verify_retained_git_directory_identity(
            &self.repository,
            OsStr::new("objects"),
            &self.objects,
            &self.objects_identity,
            &self.repository_path.join("objects"),
            "Git object directory no longer identifies the retained directory",
        )
    }

    pub(crate) fn verify_current(
        &self,
        limits: LocalSourceLimits,
    ) -> Result<(), SourceResolveError> {
        self.verify_identity()?;
        verify_cache_custody_from_open_root(
            &self.entry_root,
            self.entry
                .try_clone()
                .map_err(|error| io_error(&self.entry_root, error))?,
            CacheCustodyKind::Git,
            git_cache_custody_byte_limit(limits),
        )?;
        let actual_metadata = read_bounded_cache_record_from_open_directory(
            CacheCustodyKind::Git,
            &self.entry,
            &self.entry_root,
            Path::new(GIT_CACHE_METADATA),
            self.expected_metadata.len(),
        )?;
        if actual_metadata != self.expected_metadata {
            return Err(cache_invalid(
                &self.entry_root,
                "resolver metadata does not match the exact source locator and revision",
            ));
        }
        verify_git_repository_tree_from_open_root(&self.repository, &self.repository_path)?;
        reject_retained_git_path(
            &self.objects,
            &self.repository_path.join("objects"),
            &["info", "alternates"],
        )?;
        reject_retained_git_path(
            &self.objects,
            &self.repository_path.join("objects"),
            &["info", "http-alternates"],
        )?;
        reject_retained_git_path(&self.repository, &self.repository_path, &["commondir"])?;
        self.read_canonical_config()?;
        self.verify_identity()
    }

    pub(crate) fn read_canonical_config(&self) -> Result<Vec<u8>, SourceResolveError> {
        let config_path = self.repository_path.join("config");
        let config = read_bounded_cache_record_from_open_directory(
            CacheCustodyKind::Git,
            &self.repository,
            &self.repository_path,
            Path::new("config"),
            GIT_CONFIG_SHA256.len(),
        )?;
        if config.as_slice() != GIT_CONFIG_SHA1 && config.as_slice() != GIT_CONFIG_SHA256 {
            return Err(cache_invalid(
                &config_path,
                "local Git configuration is not the exact resolver-owned canonical file",
            ));
        }
        Ok(config)
    }

    pub(crate) fn restore_canonical_config(
        &self,
        canonical_config: &[u8],
    ) -> Result<(), SourceResolveError> {
        debug_assert!(canonical_config == GIT_CONFIG_SHA1 || canonical_config == GIT_CONFIG_SHA256);
        self.verify_identity()?;
        let result = replace_canonical_git_control_file_from_open_repository(
            &self.repository,
            &self.repository_path,
            canonical_config,
        );
        reconcile_git_cache_operation_result(result, self.verify_identity(), None)
    }

    pub(crate) fn run_git<I, S>(
        &self,
        executor: &GitExecutor,
        args: I,
    ) -> Result<(), SourceResolveError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.verify_identity()?;
        let result = run_git(
            executor,
            &self.repository_path,
            ResolverExecutionPhase::Fetch,
            args,
        );
        reconcile_git_cache_operation_result(result, self.verify_identity(), None)
    }

    pub(crate) fn run_git_stdout<I, S>(
        &self,
        executor: &GitExecutor,
        args: I,
    ) -> Result<String, SourceResolveError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.verify_identity()?;
        let result = run_git_stdout(
            executor,
            &self.repository_path,
            ResolverExecutionPhase::RepositoryInspection,
            args,
        );
        reconcile_git_cache_operation_result(result, self.verify_identity(), None)
    }

    pub(crate) fn run_git_bytes_stdout<I, S>(
        &self,
        executor: &GitExecutor,
        args: I,
    ) -> Result<Vec<u8>, SourceResolveError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.verify_identity()?;
        let result = run_git_bytes_stdout(
            executor,
            &self.repository_path,
            ResolverExecutionPhase::RepositoryInspection,
            args,
        );
        reconcile_git_cache_operation_result(result, self.verify_identity(), None)
    }

    pub(crate) fn open_or_create_snapshots(
        &self,
    ) -> Result<RetainedGitSnapshots, SourceResolveError> {
        self.verify_identity()?;
        let path = self.entry_root.join(GIT_CACHE_SNAPSHOTS);
        let name = OsStr::new(GIT_CACHE_SNAPSHOTS);
        let (directory, identity) = match self.entry.symlink_metadata(name) {
            Ok(_) => {
                let (directory, identity) = open_retained_git_directory(
                    &self.entry,
                    name,
                    &path,
                    "Git snapshot collection is not a concrete directory",
                )?;
                verify_capability_cache_node_owner_and_mode(
                    CacheCustodyKind::Git,
                    &path,
                    &identity,
                )?;
                verify_macos_open_cache_extended_acl_custody(
                    CacheCustodyKind::Git,
                    &path,
                    &directory
                        .try_clone()
                        .map_err(|error| io_error(&path, error))?
                        .into_std_file(),
                )?;
                (directory, identity)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                create_private_cache_directory(&self.entry, name)
                    .map_err(|error| io_error(&path, error))?;
                let provisional = ProvisionalCacheDirectory::new(&self.entry, name);
                let directory = retain_private_cache_directory(
                    CacheCustodyKind::Git,
                    &self.entry,
                    name,
                    &path,
                )?;
                let identity = directory
                    .dir_metadata()
                    .map_err(|error| io_error(&path, error))?;
                provisional.disarm();
                (directory, identity)
            }
            Err(error) => return Err(io_error(&path, error)),
        };
        let snapshots = RetainedGitSnapshots {
            path,
            entry: self
                .entry
                .try_clone()
                .map_err(|error| io_error(&self.entry_root, error))?,
            directory,
            identity,
        };
        snapshots.verify_identity()?;
        self.verify_identity()?;
        Ok(snapshots)
    }
}
