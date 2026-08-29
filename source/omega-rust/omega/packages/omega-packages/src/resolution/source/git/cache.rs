//! Git repository cache creation, reuse, validation, and invalidation.

use super::*;

pub(in crate::resolution::source) fn create_git_cache_entry(
    executor: &GitExecutor,
    cache_dir: &Path,
    cache_directory: &CapabilityDirectory,
    entry_root: &Path,
    entry_name: &OsStr,
    cache_identity: &str,
    locator_identity: &str,
    fetch_locator: &str,
    requested_rev: &str,
    execution_transport: GitExecutionTransport,
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    let mut pending = PendingCacheEntry::create(cache_dir, cache_directory, cache_identity)?;
    let repository = pending.root.join(GIT_CACHE_REPOSITORY);
    let empty_template = pending.root.join("empty-template");
    pending.create_private_directory("empty-template", &empty_template)?;
    pending.verify_ambient_path_identity(cache_dir)?;
    let object_format_result =
        discover_git_object_format(executor, &pending.root, fetch_locator, requested_rev);
    let object_format = reconcile_git_cache_operation_result(
        object_format_result,
        pending.verify_ambient_path_identity(cache_dir),
        None,
    )?;
    let mut init_arguments = vec![
        OsString::from("init"),
        OsString::from("--quiet"),
        OsString::from("--bare"),
    ];
    if object_format == GitObjectIdAlgorithm::Sha256 {
        init_arguments.push(OsString::from("--object-format=sha256"));
    }
    init_arguments.push(OsString::from("--template"));
    init_arguments.push(empty_template.as_os_str().to_owned());
    init_arguments.push(repository.as_os_str().to_owned());
    pending.verify_ambient_path_identity(cache_dir)?;
    let init_result = run_git(
        executor,
        &pending.root,
        ResolverExecutionPhase::RepositoryInitialization,
        init_arguments.iter(),
    );
    reconcile_git_cache_operation_result(
        init_result,
        pending.verify_ambient_path_identity(cache_dir),
        None,
    )?;
    let canonical_config = match object_format {
        GitObjectIdAlgorithm::Sha1 => GIT_CONFIG_SHA1,
        GitObjectIdAlgorithm::Sha256 => GIT_CONFIG_SHA256,
    };
    pending.verify_ambient_path_identity(cache_dir)?;
    let config_result = replace_canonical_git_control_file(
        pending.directory()?,
        OsStr::new(GIT_CACHE_REPOSITORY),
        &repository,
        canonical_config,
    );
    reconcile_git_cache_operation_result(
        config_result,
        pending.verify_ambient_path_identity(cache_dir),
        None,
    )?;
    pending
        .directory()?
        .remove_dir("empty-template")
        .map_err(|error| io_error(&empty_template, error))?;

    let metadata_path = pending.root.join(GIT_CACHE_METADATA);
    let mut metadata_options = CapabilityOpenOptions::new();
    metadata_options
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    #[cfg(unix)]
    metadata_options.mode(0o600);
    let mut metadata = pending
        .directory()?
        .open_with(GIT_CACHE_METADATA, &metadata_options)
        .map_err(|error| io_error(&metadata_path, error))?;
    #[cfg(unix)]
    {
        let mut permissions = metadata
            .metadata()
            .map_err(|error| io_error(&metadata_path, error))?
            .permissions();
        permissions.set_mode(0o600);
        metadata
            .set_permissions(permissions)
            .map_err(|error| io_error(&metadata_path, error))?;
    }
    metadata
        .write_all(&git_cache_metadata(
            locator_identity,
            requested_rev,
            execution_transport,
        ))
        .map_err(|error| io_error(&metadata_path, error))?;
    metadata
        .sync_all()
        .map_err(|error| io_error(&metadata_path, error))?;
    let metadata_custody = metadata
        .metadata()
        .map_err(|error| io_error(&metadata_path, error))?;
    verify_capability_cache_node_owner_and_mode(
        CacheCustodyKind::Git,
        &metadata_path,
        &metadata_custody,
    )?;
    #[cfg(unix)]
    {
        use cap_fs_ext::OsMetadataExt;

        if metadata_custody.mode() & 0o777 != 0o600 {
            return Err(cache_invalid(
                &metadata_path,
                "resolver metadata does not have exact private mode 0600",
            ));
        }
    }

    pending.verify_ambient_path_identity(cache_dir)?;
    let verification_result = VerifiedGitRepository::open(
        &pending.parent,
        &pending.stage_name,
        &pending.root,
        locator_identity,
        requested_rev,
        execution_transport,
        limits,
    );
    reconcile_git_cache_operation_result(
        verification_result,
        pending.verify_ambient_path_identity(cache_dir),
        None,
    )?;
    pending.publish(cache_dir, entry_root, entry_name)?;
    Ok(())
}

fn discover_git_object_format(
    executor: &GitExecutor,
    working_directory: &Path,
    url: &str,
    requested_rev: &str,
) -> Result<GitObjectIdAlgorithm, SourceResolveError> {
    if is_object_id(requested_rev) {
        return git_object_algorithm(requested_rev);
    }
    let output = run_git_bytes_stdout(
        executor,
        working_directory,
        ResolverExecutionPhase::TransportDiscovery,
        [
            OsStr::new("ls-remote"),
            OsStr::new("--symref"),
            OsStr::new("--"),
            OsStr::new(url),
            OsStr::new("HEAD"),
            OsStr::new(requested_rev),
        ],
    )?;
    parse_git_remote_object_format(&output, working_directory)
}

pub(in crate::resolution::source) fn parse_git_remote_object_format(
    output: &[u8],
    working_directory: &Path,
) -> Result<GitObjectIdAlgorithm, SourceResolveError> {
    let mut selected = None;
    for line in output.split(|byte| *byte == b'\n') {
        if line.is_empty() || line.starts_with(b"ref: ") {
            continue;
        }
        let Some(separator) = line.iter().position(|byte| *byte == b'\t') else {
            return Err(cache_invalid(
                working_directory,
                "Git object-format discovery returned a malformed row",
            ));
        };
        let oid = std::str::from_utf8(&line[..separator]).map_err(|_| {
            cache_invalid(
                working_directory,
                "Git object-format discovery returned a non-ASCII object ID",
            )
        })?;
        let algorithm = git_object_algorithm(oid)?;
        if selected.is_some_and(|selected| selected != algorithm) {
            return Err(cache_invalid(
                working_directory,
                "Git object-format discovery returned mixed hash algorithms",
            ));
        }
        selected = Some(algorithm);
    }
    selected.ok_or_else(|| {
        cache_invalid(
            working_directory,
            "Git object-format discovery returned no selected object ID",
        )
    })
}

pub(in crate::resolution::source) struct VerifiedGitRepository {
    pub(in crate::resolution::source) entry_root: PathBuf,
    pub(in crate::resolution::source) repository_path: PathBuf,
    pub(in crate::resolution::source) entry_name: OsString,
    pub(in crate::resolution::source) expected_metadata: Vec<u8>,
    pub(in crate::resolution::source) cache_parent: CapabilityDirectory,
    pub(in crate::resolution::source) entry: CapabilityDirectory,
    pub(in crate::resolution::source) repository: CapabilityDirectory,
    pub(in crate::resolution::source) objects: CapabilityDirectory,
    pub(in crate::resolution::source) entry_identity: CapabilityMetadata,
    pub(in crate::resolution::source) repository_identity: CapabilityMetadata,
    pub(in crate::resolution::source) objects_identity: CapabilityMetadata,
}

impl VerifiedGitRepository {
    pub(in crate::resolution::source) fn open(
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

    pub(in crate::resolution::source) fn path(&self) -> &Path {
        &self.repository_path
    }

    pub(in crate::resolution::source) fn verify_identity(&self) -> Result<(), SourceResolveError> {
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

    pub(in crate::resolution::source) fn verify_current(
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

    pub(in crate::resolution::source) fn read_canonical_config(
        &self,
    ) -> Result<Vec<u8>, SourceResolveError> {
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

    pub(in crate::resolution::source) fn restore_canonical_config(
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

    pub(in crate::resolution::source) fn run_git<I, S>(
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

    pub(in crate::resolution::source) fn run_git_stdout<I, S>(
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

    pub(in crate::resolution::source) fn run_git_bytes_stdout<I, S>(
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

    pub(in crate::resolution::source) fn open_or_create_snapshots(
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
                verify_windows_open_cache_custody(
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

pub(in crate::resolution::source) struct RetainedGitSnapshots {
    pub(in crate::resolution::source) path: PathBuf,
    pub(in crate::resolution::source) entry: CapabilityDirectory,
    pub(in crate::resolution::source) directory: CapabilityDirectory,
    pub(in crate::resolution::source) identity: CapabilityMetadata,
}

impl RetainedGitSnapshots {
    pub(in crate::resolution::source) fn verify_identity(&self) -> Result<(), SourceResolveError> {
        verify_retained_git_directory_identity(
            &self.entry,
            OsStr::new(GIT_CACHE_SNAPSHOTS),
            &self.directory,
            &self.identity,
            &self.path,
            "Git snapshot collection no longer identifies the retained directory",
        )
    }

    pub(in crate::resolution::source) fn publication_exists(
        &self,
        publication: &Path,
    ) -> Result<bool, SourceResolveError> {
        self.verify_identity()?;
        let name = direct_cache_child_name(CacheCustodyKind::Git, &self.path, publication)?;
        match self.directory.symlink_metadata(name) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                Err(cache_invalid(
                    publication,
                    "Git snapshot publication is not a concrete directory",
                ))
            }
            Ok(_) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(io_error(publication, error)),
        }
    }
}

fn open_retained_git_directory(
    parent: &CapabilityDirectory,
    name: &OsStr,
    path: &Path,
    message: &str,
) -> Result<(CapabilityDirectory, CapabilityMetadata), SourceResolveError> {
    let classified = parent
        .symlink_metadata(name)
        .map_err(|error| io_error(path, error))?;
    if classified.file_type().is_symlink() || !classified.is_dir() {
        return Err(cache_invalid(path, message));
    }
    let directory = parent
        .open_dir_nofollow(name)
        .map_err(|error| cache_invalid(path, error.to_string()))?;
    let opened = directory
        .dir_metadata()
        .map_err(|error| io_error(path, error))?;
    if !opened.is_dir() || !same_capability_file_identity(&classified, &opened) {
        return Err(cache_invalid(
            path,
            "Git directory changed between classification and no-follow open",
        ));
    }
    Ok((directory, opened))
}

fn verify_retained_git_directory_identity(
    parent: &CapabilityDirectory,
    name: &OsStr,
    retained: &CapabilityDirectory,
    expected: &CapabilityMetadata,
    path: &Path,
    message: &str,
) -> Result<(), SourceResolveError> {
    let named = parent
        .symlink_metadata(name)
        .map_err(|error| io_error(path, error))?;
    let opened = retained
        .dir_metadata()
        .map_err(|error| io_error(path, error))?;
    if named.file_type().is_symlink()
        || !named.is_dir()
        || !opened.is_dir()
        || !same_capability_file_identity(expected, &named)
        || !same_capability_file_identity(expected, &opened)
    {
        return Err(cache_invalid(path, message));
    }
    Ok(())
}

fn verify_git_repository_tree_from_open_root(
    repository: &CapabilityDirectory,
    repository_path: &Path,
) -> Result<(), SourceResolveError> {
    let root_metadata = repository
        .dir_metadata()
        .map_err(|error| io_error(repository_path, error))?;
    let mut pending = vec![(
        PathBuf::new(),
        repository_path.to_path_buf(),
        root_metadata,
        0usize,
    )];
    let mut observed = 0usize;
    while let Some((relative_path, path, classified, depth)) = pending.pop() {
        observed = observed
            .checked_add(1)
            .ok_or_else(|| cache_invalid(&path, "Git repository entry count overflowed"))?;
        if observed > CACHE_CUSTODY_ENTRY_LIMIT {
            return Err(cache_invalid(
                repository_path,
                format!("Git repository exceeds its {CACHE_CUSTODY_ENTRY_LIMIT}-entry ceiling"),
            ));
        }
        let directory = open_cache_custody_directory(
            repository,
            &relative_path,
            &path,
            &classified,
            CacheCustodyKind::Git,
        )?;
        for child in directory
            .entries()
            .map_err(|error| io_error(&path, error))?
        {
            let child = child.map_err(|error| io_error(&path, error))?;
            let name = child.file_name();
            let child_path = path.join(&name);
            let metadata = directory
                .symlink_metadata(&name)
                .map_err(|error| io_error(&child_path, error))?;
            if metadata.file_type().is_symlink() {
                return Err(cache_invalid(
                    &child_path,
                    "symlinks are forbidden in the native Git repository",
                ));
            }
            if metadata.is_file() {
                verify_retained_git_regular_file(&directory, &name, &child_path, &metadata)?;
                observed = observed.checked_add(1).ok_or_else(|| {
                    cache_invalid(&child_path, "Git repository entry count overflowed")
                })?;
            } else if metadata.is_dir() {
                let child_depth = depth
                    .checked_add(1)
                    .ok_or_else(|| cache_invalid(&child_path, "Git repository depth overflowed"))?;
                if child_depth > CACHE_CUSTODY_DEPTH_LIMIT {
                    return Err(cache_invalid(
                        &child_path,
                        format!(
                            "Git repository exceeds its {CACHE_CUSTODY_DEPTH_LIMIT}-level depth ceiling"
                        ),
                    ));
                }
                pending.push((relative_path.join(&name), child_path, metadata, child_depth));
            } else {
                return Err(cache_invalid(
                    &child_path,
                    "native Git repository contains an unsupported filesystem entry kind",
                ));
            }
            if observed
                .checked_add(pending.len())
                .is_none_or(|total| total > CACHE_CUSTODY_ENTRY_LIMIT)
            {
                return Err(cache_invalid(
                    repository_path,
                    format!("Git repository exceeds its {CACHE_CUSTODY_ENTRY_LIMIT}-entry ceiling"),
                ));
            }
        }
    }
    Ok(())
}

fn verify_retained_git_regular_file(
    parent: &CapabilityDirectory,
    name: &OsStr,
    path: &Path,
    classified: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    let mut options = CapabilityOpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = parent
        .open_with(name, &options)
        .map_err(|error| cache_invalid(path, error.to_string()))?;
    let opened = file.metadata().map_err(|error| io_error(path, error))?;
    if !opened.is_file() || !same_capability_file_identity(classified, &opened) {
        return Err(cache_invalid(
            path,
            "Git repository file changed between classification and no-follow open",
        ));
    }
    verify_git_regular_file_link_count(path, &opened)
}

#[cfg(unix)]
fn verify_git_regular_file_link_count(
    path: &Path,
    metadata: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    use cap_fs_ext::OsMetadataExt;

    if metadata.nlink() != 1 {
        return Err(cache_invalid(
            path,
            "multiply-linked files are forbidden in the native Git repository",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_git_regular_file_link_count(
    _path: &Path,
    _metadata: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

fn reject_retained_git_path(
    root: &CapabilityDirectory,
    root_path: &Path,
    components: &[&str],
) -> Result<(), SourceResolveError> {
    let Some((leaf, parents)) = components.split_last() else {
        return Err(cache_invalid(root_path, "forbidden Git path is empty"));
    };
    let mut directory = root
        .try_clone()
        .map_err(|error| io_error(root_path, error))?;
    let mut path = root_path.to_path_buf();
    for parent in parents {
        path.push(parent);
        let metadata = match directory.symlink_metadata(parent) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(io_error(&path, error)),
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(cache_invalid(
                &path,
                "cannot prove forbidden Git path absent beneath a non-directory",
            ));
        }
        let opened = directory
            .open_dir_nofollow(parent)
            .map_err(|error| cache_invalid(&path, error.to_string()))?;
        let opened_metadata = opened
            .dir_metadata()
            .map_err(|error| io_error(&path, error))?;
        if !same_capability_file_identity(&metadata, &opened_metadata) {
            return Err(cache_invalid(
                &path,
                "Git directory changed while checking forbidden indirection",
            ));
        }
        directory = opened;
    }
    path.push(leaf);
    match directory.symlink_metadata(leaf) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(&path, error)),
        Ok(_) => Err(cache_invalid(
            &path,
            "external Git object or directory indirection is forbidden",
        )),
    }
}

#[cfg(test)]
pub(in crate::resolution::source) fn invalidate_git_cache_entry_from_retained_parent(
    entry_root: &Path,
) -> Result<(), SourceResolveError> {
    let cache_root = entry_root
        .parent()
        .ok_or_else(|| cache_invalid(entry_root, "Git cache entry has no cache parent"))?;
    verify_git_cache_root_custody(cache_root)?;
    let cache_directory = open_absolute_directory_nofollow(cache_root)
        .map_err(|error| cache_invalid(cache_root, error.to_string()))?;
    let entry_name = direct_cache_child_name(CacheCustodyKind::Git, cache_root, entry_root)?;
    invalidate_git_cache_entry_from_open_parent(
        cache_root,
        &cache_directory,
        entry_name,
        entry_root,
    )
}

pub(in crate::resolution::source) fn invalidate_git_cache_entry_from_open_parent(
    cache_root: &Path,
    cache_directory: &CapabilityDirectory,
    entry_name: &OsStr,
    entry_root: &Path,
) -> Result<(), SourceResolveError> {
    let classified = cache_directory
        .symlink_metadata(entry_name)
        .map_err(|error| io_error(entry_root, error))?;
    if classified.file_type().is_symlink() || !classified.is_dir() {
        return Err(cache_invalid(
            entry_root,
            "Git cache invalidation target is not a concrete directory",
        ));
    }
    let entry_directory = cache_directory
        .open_dir_nofollow(entry_name)
        .map_err(|error| cache_invalid(entry_root, error.to_string()))?;
    let opened = entry_directory
        .dir_metadata()
        .map_err(|error| io_error(entry_root, error))?;
    if !same_capability_file_identity(&classified, &opened) {
        return Err(cache_invalid(
            entry_root,
            "Git cache entry changed while opening it for invalidation",
        ));
    }
    entry_directory
        .remove_file(GIT_CACHE_METADATA)
        .map_err(|error| io_error(&entry_root.join(GIT_CACHE_METADATA), error))?;
    cache_directory
        .try_clone()
        .map_err(|error| io_error(cache_root, error))?
        .into_std_file()
        .sync_all()
        .map_err(|error| io_error(cache_root, error))
}

pub(in crate::resolution::source) fn git_cache_identity(
    url: &str,
    requested_rev: &str,
    execution_transport: GitExecutionTransport,
) -> String {
    let mut hasher = Sha256::new();
    hash_bytes(&mut hasher, GIT_CACHE_POLICY);
    hash_bytes(&mut hasher, url.as_bytes());
    hash_bytes(&mut hasher, requested_rev.as_bytes());
    hash_bytes(&mut hasher, execution_transport.cache_tag());
    format_sha256(&hasher.finalize())
}

pub(in crate::resolution::source) fn git_cache_metadata(
    url: &str,
    requested_rev: &str,
    execution_transport: GitExecutionTransport,
) -> Vec<u8> {
    let mut metadata = Vec::new();
    metadata.extend_from_slice(GIT_CACHE_POLICY);
    append_framed_bytes(&mut metadata, url.as_bytes());
    append_framed_bytes(&mut metadata, requested_rev.as_bytes());
    append_framed_bytes(&mut metadata, execution_transport.cache_tag());
    metadata
}

pub(in crate::resolution::source) fn append_framed_bytes(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    output.extend_from_slice(bytes);
}

pub(in crate::resolution::source) fn cache_invalid(
    path: &Path,
    message: impl Into<String>,
) -> SourceResolveError {
    SourceResolveError::GitCacheInvalid {
        path: path.to_path_buf(),
        message: message.into(),
    }
}

pub(in crate::resolution::source) fn local_snapshot_invalid(
    path: &Path,
    message: impl Into<String>,
) -> SourceResolveError {
    SourceResolveError::LocalSnapshotInvalid {
        path: path.to_path_buf(),
        message: message.into(),
    }
}
