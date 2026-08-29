//! Filesystem custody shared by local snapshots and Git cache entries.

use super::*;
pub(in crate::resolution::source) struct CacheEntryLock {
    pub(in crate::resolution::source) file: File,
    pub(in crate::resolution::source) parent: CapabilityDirectory,
    pub(in crate::resolution::source) kind: CacheCustodyKind,
    pub(in crate::resolution::source) path: PathBuf,
    pub(in crate::resolution::source) lock_name: OsString,
}

impl CacheEntryLock {
    pub(in crate::resolution::source) fn open_retained(
        kind: CacheCustodyKind,
        path: &Path,
    ) -> Result<(File, CapabilityDirectory, OsString), SourceResolveError> {
        let parent_path = path.parent().ok_or_else(|| {
            cache_custody_invalid(kind, path, "cache lock has no publication parent")
        })?;
        verify_cache_custody_root(parent_path, kind)?;
        let parent = open_absolute_directory_nofollow(parent_path)
            .map_err(|error| cache_custody_invalid(kind, parent_path, error.to_string()))?;
        let lock_name = direct_cache_child_name(kind, parent_path, path)?.to_os_string();
        let mut options = CapabilityOpenOptions::new();
        options
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .follow(FollowSymlinks::No);
        #[cfg(unix)]
        options.mode(0o600);
        let capability_file = parent.open_with(&lock_name, &options).map_err(|error| {
            cache_custody_invalid(
                kind,
                path,
                format!("could not open cache lock without following links: {error}"),
            )
        })?;
        let handle_metadata = capability_file
            .metadata()
            .map_err(|error| io_error(path, error))?;
        let path_metadata = parent
            .symlink_metadata(&lock_name)
            .map_err(|error| io_error(path, error))?;
        if !handle_metadata.is_file()
            || path_metadata.file_type().is_symlink()
            || !path_metadata.is_file()
            || !same_capability_file_identity(&handle_metadata, &path_metadata)
        {
            return Err(cache_custody_invalid(
                kind,
                path,
                "cache lock is not a stable regular file beneath its retained parent",
            ));
        }
        verify_capability_cache_node_owner_and_mode(kind, path, &path_metadata)?;
        let file = capability_file.into_std();
        verify_macos_open_cache_extended_acl_custody(kind, path, &file)?;
        verify_windows_open_cache_custody(kind, path, &file)?;
        Ok((file, parent, lock_name))
    }

    #[cfg(test)]
    pub(in crate::resolution::source) fn open_git(path: &Path) -> Result<File, SourceResolveError> {
        let (file, _, _) = Self::open_retained(CacheCustodyKind::Git, path)?;
        Ok(file)
    }

    pub(in crate::resolution::source) fn acquire_with_git_budget(
        path: &Path,
        executor: &GitExecutor,
    ) -> Result<Self, SourceResolveError> {
        let (file, parent, lock_name) = Self::open_retained(CacheCustodyKind::Git, path)?;
        loop {
            executor.verify_budget()?;
            match file.try_lock() {
                Ok(()) => break,
                Err(std::fs::TryLockError::WouldBlock) => {
                    let remaining = executor.remaining_time()?;
                    std::thread::sleep(PROCESS_POLL_INTERVAL.min(remaining));
                }
                Err(std::fs::TryLockError::Error(error)) => {
                    return Err(io_error(path, error));
                }
            }
        }
        if let Err(error) = executor.verify_budget() {
            let _ = file.unlock();
            return Err(error);
        }
        verify_cache_lock_path_identity(CacheCustodyKind::Git, path, &parent, &lock_name, &file)?;
        Ok(Self {
            file,
            parent,
            kind: CacheCustodyKind::Git,
            path: path.to_path_buf(),
            lock_name,
        })
    }

    #[cfg(test)]
    pub(in crate::resolution::source) fn acquire(path: &Path) -> Result<Self, SourceResolveError> {
        let (file, parent, lock_name) = Self::open_retained(CacheCustodyKind::Git, path)?;
        file.lock().map_err(|error| io_error(path, error))?;
        verify_cache_lock_path_identity(CacheCustodyKind::Git, path, &parent, &lock_name, &file)?;
        Ok(Self {
            file,
            parent,
            kind: CacheCustodyKind::Git,
            path: path.to_path_buf(),
            lock_name,
        })
    }

    pub(in crate::resolution::source) fn acquire_local(
        path: &Path,
    ) -> Result<Self, SourceResolveError> {
        Self::acquire_local_with_timeout(path, LOCAL_SNAPSHOT_LOCK_TIMEOUT)
    }

    pub(in crate::resolution::source) fn acquire_local_with_timeout(
        path: &Path,
        timeout: Duration,
    ) -> Result<Self, SourceResolveError> {
        let (file, parent, lock_name) = Self::open_retained(CacheCustodyKind::LocalSnapshot, path)?;
        let started = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => break,
                Err(std::fs::TryLockError::WouldBlock) => {
                    let Some(remaining) = timeout.checked_sub(started.elapsed()) else {
                        return Err(local_snapshot_lock_timed_out(path, timeout));
                    };
                    if remaining.is_zero() {
                        return Err(local_snapshot_lock_timed_out(path, timeout));
                    }
                    std::thread::sleep(PROCESS_POLL_INTERVAL.min(remaining));
                }
                Err(std::fs::TryLockError::Error(error)) => {
                    return Err(io_error(path, error));
                }
            }
        }
        if started.elapsed() >= timeout {
            let _ = file.unlock();
            return Err(local_snapshot_lock_timed_out(path, timeout));
        }
        verify_cache_lock_path_identity(
            CacheCustodyKind::LocalSnapshot,
            path,
            &parent,
            &lock_name,
            &file,
        )?;
        Ok(Self {
            file,
            parent,
            kind: CacheCustodyKind::LocalSnapshot,
            path: path.to_path_buf(),
            lock_name,
        })
    }

    pub(in crate::resolution::source) fn parent(&self) -> &CapabilityDirectory {
        &self.parent
    }

    pub(in crate::resolution::source) fn verify_path_identity(
        &self,
    ) -> Result<(), SourceResolveError> {
        verify_cache_lock_path_identity(
            self.kind,
            &self.path,
            &self.parent,
            &self.lock_name,
            &self.file,
        )
    }
}

fn local_snapshot_lock_timed_out(path: &Path, timeout: Duration) -> SourceResolveError {
    SourceResolveError::LocalSnapshotLockTimedOut {
        path: path.to_path_buf(),
        timeout_millis: u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX),
    }
}

pub(in crate::resolution::source) fn verify_cache_lock_path_identity(
    kind: CacheCustodyKind,
    path: &Path,
    parent: &CapabilityDirectory,
    lock_name: &OsStr,
    file: &File,
) -> Result<(), SourceResolveError> {
    let path_metadata = parent
        .symlink_metadata(lock_name)
        .map_err(|error| io_error(path, error))?;
    if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache lock was replaced while being acquired",
        ));
    }
    let handle_metadata = file.metadata().map_err(|error| io_error(path, error))?;
    if !handle_metadata.is_file()
        || !same_std_and_capability_file_identity(&handle_metadata, &path_metadata)
    {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache lock path does not identify the locked file",
        ));
    }
    verify_capability_cache_node_owner_and_mode(kind, path, &path_metadata)?;
    verify_macos_open_cache_extended_acl_custody(kind, path, file)?;
    verify_windows_open_cache_custody(kind, path, file)?;

    let parent_path = path
        .parent()
        .ok_or_else(|| cache_custody_invalid(kind, path, "cache lock has no publication parent"))?;
    verify_retained_cache_parent_path(kind, parent_path, parent)
}

pub(in crate::resolution::source) fn verify_retained_cache_parent_path(
    kind: CacheCustodyKind,
    parent_path: &Path,
    retained_parent: &CapabilityDirectory,
) -> Result<(), SourceResolveError> {
    verify_cache_custody_root(parent_path, kind)?;
    let current_parent = open_absolute_directory_nofollow(parent_path)
        .map_err(|error| cache_custody_invalid(kind, parent_path, error.to_string()))?;
    let retained_metadata = retained_parent
        .dir_metadata()
        .map_err(|error| io_error(parent_path, error))?;
    let current_metadata = current_parent
        .dir_metadata()
        .map_err(|error| io_error(parent_path, error))?;
    if !same_capability_file_identity(&retained_metadata, &current_metadata) {
        return Err(cache_custody_invalid(
            kind,
            parent_path,
            "cache parent pathname no longer identifies the retained directory",
        ));
    }
    Ok(())
}

pub(in crate::resolution::source) fn same_std_and_capability_file_identity(
    left: &std::fs::Metadata,
    right: &CapabilityMetadata,
) -> bool {
    use cap_fs_ext::MetadataExt;

    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(test)]
pub(in crate::resolution::source) fn verify_cache_lock_path_identity_for_test(
    kind: CacheCustodyKind,
    path: &Path,
    file: &File,
) -> Result<(), SourceResolveError> {
    let parent_path = path.parent().expect("test cache lock has a parent");
    let canonical_parent = parent_path
        .canonicalize()
        .map_err(|error| io_error(parent_path, error))?;
    let lock_name = path.file_name().expect("test cache lock has a name");
    let canonical_path = canonical_parent.join(lock_name);
    let parent = open_absolute_directory_nofollow(&canonical_parent)
        .map_err(|error| io_error(&canonical_parent, error))?;
    verify_cache_lock_path_identity(kind, &canonical_path, &parent, lock_name, file)
}

impl Drop for CacheEntryLock {
    fn drop(&mut self) {
        // Keep the inode in place: unlinking a lock file lets a waiter lock the old inode while a
        // newcomer locks a replacement. Closing this handle releases the advisory lock safely.
        let _ = self.file.unlock();
    }
}
