use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const GIT_CACHE_POLICY: &[u8] = b"omega-git-cache-v2";
const GIT_CACHE_METADATA: &str = "source.identity";
const GIT_CACHE_REPOSITORY: &str = "repository";
const GIT_ORIGIN_FETCH: &str = "+refs/heads/*:refs/remotes/origin/*";
static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalSourceLimits {
    pub max_files: usize,
    pub max_bytes: u64,
    pub max_depth: usize,
}

impl Default for LocalSourceLimits {
    fn default() -> Self {
        Self {
            max_files: 4096,
            max_bytes: 256 * 1024 * 1024,
            max_depth: 64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLocalSource {
    pub root: PathBuf,
    pub file_count: usize,
    pub byte_count: u64,
    pub content_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitSourceSpec {
    pub url: String,
    pub rev: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedGitSource {
    pub url: String,
    pub requested_rev: String,
    pub commit: String,
    pub tree: String,
    pub checkout_root: PathBuf,
    pub local: ResolvedLocalSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceResolveError {
    Io {
        path: PathBuf,
        message: String,
    },
    NotDirectory {
        path: PathBuf,
    },
    TooManyFiles {
        limit: usize,
    },
    TooManyBytes {
        limit: u64,
    },
    TooDeep {
        path: PathBuf,
        limit: usize,
    },
    SymlinkEscapesRoot {
        link: PathBuf,
        target: PathBuf,
    },
    SymlinkTargetsExcludedMetadata {
        link: PathBuf,
        target: PathBuf,
    },
    UnsupportedFileType {
        path: PathBuf,
    },
    Git {
        operation: String,
        status: Option<i32>,
        stderr: String,
    },
    GitSubmodulesUnsupported {
        path: PathBuf,
    },
    GitCacheInvalid {
        path: PathBuf,
        message: String,
    },
}

impl fmt::Display for SourceResolveError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, message } => write!(output, "{}: {message}", path.display()),
            Self::NotDirectory { path } => {
                write!(
                    output,
                    "source root `{}` is not a directory",
                    path.display()
                )
            }
            Self::TooManyFiles { limit } => {
                write!(output, "source root exceeds file limit of {limit}")
            }
            Self::TooManyBytes { limit } => {
                write!(output, "source root exceeds byte limit of {limit}")
            }
            Self::TooDeep { path, limit } => {
                write!(
                    output,
                    "source path `{}` exceeds traversal depth limit of {limit}",
                    path.display()
                )
            }
            Self::SymlinkEscapesRoot { link, target } => write!(
                output,
                "source symlink `{}` resolves outside package root to `{}`",
                link.display(),
                target.display()
            ),
            Self::SymlinkTargetsExcludedMetadata { link, target } => write!(
                output,
                "source symlink `{}` targets excluded repository metadata at `{}`",
                link.display(),
                target.display()
            ),
            Self::UnsupportedFileType { path } => write!(
                output,
                "source path `{}` has an unsupported filesystem entry type",
                path.display()
            ),
            Self::Git {
                operation,
                status,
                stderr,
            } => write!(
                output,
                "git {operation} failed with status {:?}: {}",
                status,
                stderr.trim()
            ),
            Self::GitSubmodulesUnsupported { path } => write!(
                output,
                "git source `{}` declares submodules; submodules must become explicit package edges before they are supported",
                path.display()
            ),
            Self::GitCacheInvalid { path, message } => write!(
                output,
                "git cache entry `{}` is invalid: {message}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for SourceResolveError {}

pub fn resolve_local_source(
    root: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSource, SourceResolveError> {
    let requested_root = root.as_ref();
    let root = requested_root
        .canonicalize()
        .map_err(|error| io_error(requested_root, error))?;
    if !root.is_dir() {
        return Err(SourceResolveError::NotDirectory { path: root });
    }

    let mut entries = Vec::new();
    let mut visited_dirs = BTreeSet::new();
    visit_directory(
        &root,
        PathBuf::new(),
        0,
        &root,
        limits,
        &mut visited_dirs,
        &mut entries,
    )?;
    entries.sort_by(|left, right| left.relative_bytes.cmp(&right.relative_bytes));

    let mut hasher = Sha256::new();
    hasher.update(b"omega-local-source-v2\0");
    hash_length(&mut hasher, entries.len() as u64);
    let mut byte_count = 0_u64;
    for entry in &entries {
        hasher.update(b"entry");
        hash_bytes(&mut hasher, &entry.relative_bytes);
        match &entry.kind {
            SourceEntryKind::File { path } => {
                let remaining = limits.max_bytes.checked_sub(byte_count).ok_or(
                    SourceResolveError::TooManyBytes {
                        limit: limits.max_bytes,
                    },
                )?;
                let (bytes, executable) = read_file_bounded(path, remaining, limits.max_bytes)?;
                byte_count = byte_count.checked_add(bytes.len() as u64).ok_or(
                    SourceResolveError::TooManyBytes {
                        limit: limits.max_bytes,
                    },
                )?;
                hasher.update(b"file");
                hasher.update([u8::from(executable)]);
                hash_bytes(&mut hasher, &bytes);
            }
            SourceEntryKind::Symlink { target_bytes } => {
                hasher.update(b"symlink");
                hash_bytes(&mut hasher, target_bytes);
            }
        }
    }

    Ok(ResolvedLocalSource {
        root,
        file_count: entries.len(),
        byte_count,
        content_identity: format_sha256(&hasher.finalize()),
    })
}

pub fn resolve_git_source(
    spec: &GitSourceSpec,
    cache_dir: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    let requested_rev = spec.rev.clone().unwrap_or_else(|| "HEAD".to_owned());
    let cache_dir = cache_dir.as_ref();
    std::fs::create_dir_all(cache_dir).map_err(|error| io_error(cache_dir, error))?;
    let cache_identity = git_cache_identity(&spec.url, &requested_rev);
    let entry_root = cache_dir.join(format!("git-{cache_identity}"));
    let lock_path = cache_dir.join(format!("git-{cache_identity}.lock"));
    let _entry_lock = CacheEntryLock::acquire(&lock_path)?;

    if entry_root.exists() {
        if let Err(error) = verify_git_cache_entry(&entry_root, &spec.url, &requested_rev) {
            invalidate_git_cache_entry(&entry_root);
            return Err(error);
        }
    } else {
        create_git_cache_entry(
            cache_dir,
            &entry_root,
            &cache_identity,
            &spec.url,
            &requested_rev,
        )?;
    }

    let result = resolve_verified_git_cache_entry(&entry_root, &spec.url, &requested_rev, limits);
    if result.is_err() {
        invalidate_git_cache_entry(&entry_root);
    }
    result
}

fn resolve_verified_git_cache_entry(
    entry_root: &Path,
    url: &str,
    requested_rev: &str,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    verify_git_cache_entry(entry_root, url, requested_rev)?;
    let checkout_root = entry_root.join(GIT_CACHE_REPOSITORY);

    run_git([
        OsStr::new("-C"),
        checkout_root.as_os_str(),
        OsStr::new("fetch"),
        OsStr::new("--quiet"),
        OsStr::new("--no-tags"),
        OsStr::new("--no-recurse-submodules"),
        OsStr::new("--"),
        OsStr::new("origin"),
        OsStr::new(requested_rev),
    ])?;
    verify_git_cache_entry(entry_root, url, requested_rev)?;

    let commit = run_git_stdout([
        OsStr::new("-C"),
        checkout_root.as_os_str(),
        OsStr::new("rev-parse"),
        OsStr::new("--verify"),
        OsStr::new("FETCH_HEAD^{commit}"),
    ])?;
    let commit = commit.trim().to_owned();
    let tree = run_git_stdout([
        OsStr::new("-C"),
        checkout_root.as_os_str(),
        OsStr::new("rev-parse"),
        OsStr::new("--verify"),
        OsStr::new(&format!("{commit}^{{tree}}")),
    ])?;
    let tree = tree.trim().to_owned();

    let gitmodules = run_git_bytes_stdout([
        OsStr::new("-C"),
        checkout_root.as_os_str(),
        OsStr::new("ls-tree"),
        OsStr::new("-z"),
        OsStr::new("--name-only"),
        OsStr::new(&commit),
        OsStr::new("--"),
        OsStr::new(".gitmodules"),
    ])?;
    if !gitmodules.is_empty() {
        return Err(SourceResolveError::GitSubmodulesUnsupported {
            path: checkout_root.join(".gitmodules"),
        });
    }
    let tree_entries = run_git_bytes_stdout([
        OsStr::new("-C"),
        checkout_root.as_os_str(),
        OsStr::new("ls-tree"),
        OsStr::new("-r"),
        OsStr::new("-z"),
        OsStr::new(&commit),
    ])?;
    if tree_entries
        .split(|byte| *byte == 0)
        .any(|entry| entry.starts_with(b"160000 "))
    {
        return Err(SourceResolveError::GitSubmodulesUnsupported {
            path: checkout_root.to_path_buf(),
        });
    }

    verify_git_cache_entry(entry_root, url, requested_rev)?;
    run_git([
        OsStr::new("-C"),
        checkout_root.as_os_str(),
        OsStr::new("checkout"),
        OsStr::new("--quiet"),
        OsStr::new("--force"),
        OsStr::new("--detach"),
        OsStr::new(&commit),
    ])?;
    run_git([
        OsStr::new("-C"),
        checkout_root.as_os_str(),
        OsStr::new("clean"),
        OsStr::new("--quiet"),
        OsStr::new("-ffdx"),
    ])?;
    verify_git_cache_entry(entry_root, url, requested_rev)?;
    let local = resolve_local_source(&checkout_root, limits)?;
    Ok(ResolvedGitSource {
        url: url.to_owned(),
        requested_rev: requested_rev.to_owned(),
        commit,
        tree,
        checkout_root,
        local,
    })
}

fn create_git_cache_entry(
    cache_dir: &Path,
    entry_root: &Path,
    cache_identity: &str,
    url: &str,
    requested_rev: &str,
) -> Result<(), SourceResolveError> {
    let mut pending = PendingCacheEntry::create(cache_dir, cache_identity)?;
    let repository = pending.root.join(GIT_CACHE_REPOSITORY);
    let empty_template = pending.root.join("empty-template");
    std::fs::create_dir(&empty_template).map_err(|error| io_error(&empty_template, error))?;
    run_git([
        OsStr::new("init"),
        OsStr::new("--quiet"),
        OsStr::new("--template"),
        empty_template.as_os_str(),
        repository.as_os_str(),
    ])?;
    run_git([
        OsStr::new("-C"),
        repository.as_os_str(),
        OsStr::new("config"),
        OsStr::new("--local"),
        OsStr::new("remote.origin.url"),
        OsStr::new(url),
    ])?;
    run_git([
        OsStr::new("-C"),
        repository.as_os_str(),
        OsStr::new("config"),
        OsStr::new("--local"),
        OsStr::new("remote.origin.fetch"),
        OsStr::new(GIT_ORIGIN_FETCH),
    ])?;
    std::fs::remove_dir(&empty_template).map_err(|error| io_error(&empty_template, error))?;

    let metadata_path = pending.root.join(GIT_CACHE_METADATA);
    let mut metadata = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&metadata_path)
        .map_err(|error| io_error(&metadata_path, error))?;
    metadata
        .write_all(&git_cache_metadata(url, requested_rev))
        .map_err(|error| io_error(&metadata_path, error))?;
    metadata
        .sync_all()
        .map_err(|error| io_error(&metadata_path, error))?;

    verify_git_cache_entry(&pending.root, url, requested_rev)?;
    std::fs::rename(&pending.root, entry_root).map_err(|error| io_error(entry_root, error))?;
    pending.published = true;
    Ok(())
}

fn verify_git_cache_entry(
    entry_root: &Path,
    url: &str,
    requested_rev: &str,
) -> Result<(), SourceResolveError> {
    require_real_directory(entry_root, "cache entry root is not a real directory")?;
    let metadata_path = entry_root.join(GIT_CACHE_METADATA);
    require_regular_file(&metadata_path, "resolver metadata is not a regular file")?;
    let expected_metadata = git_cache_metadata(url, requested_rev);
    let metadata_size = std::fs::symlink_metadata(&metadata_path)
        .map_err(|error| io_error(&metadata_path, error))?
        .len();
    if metadata_size != expected_metadata.len() as u64 {
        return Err(cache_invalid(
            &metadata_path,
            "resolver metadata has an unexpected length",
        ));
    }
    let actual_metadata =
        std::fs::read(&metadata_path).map_err(|error| io_error(&metadata_path, error))?;
    if actual_metadata != expected_metadata {
        return Err(cache_invalid(
            entry_root,
            "resolver metadata does not match the exact source locator and revision",
        ));
    }

    let repository = entry_root.join(GIT_CACHE_REPOSITORY);
    require_real_directory(&repository, "repository is not a real directory")?;
    require_real_directory(
        &repository.join(".git"),
        "Git directory is not a real directory",
    )?;
    for forbidden in [
        repository.join(".git/objects/info/alternates"),
        repository.join(".git/objects/info/http-alternates"),
        repository.join(".git/commondir"),
    ] {
        if std::fs::symlink_metadata(&forbidden).is_ok() {
            return Err(cache_invalid(
                &forbidden,
                "external Git object or directory indirection is forbidden",
            ));
        }
    }

    let config_path = repository.join(".git/config");
    require_regular_file(
        &config_path,
        "local Git configuration is not a regular file",
    )?;
    if std::fs::symlink_metadata(&config_path)
        .map_err(|error| io_error(&config_path, error))?
        .len()
        > 64 * 1024
    {
        return Err(cache_invalid(
            &config_path,
            "local Git configuration exceeds the resolver limit",
        ));
    }
    let config = run_git_bytes_stdout([
        OsStr::new("-C"),
        repository.as_os_str(),
        OsStr::new("config"),
        OsStr::new("--local"),
        OsStr::new("--no-includes"),
        OsStr::new("--null"),
        OsStr::new("--list"),
    ])?;
    verify_local_git_config(entry_root, url, &config)?;

    let origin = run_git_bytes_stdout([
        OsStr::new("-C"),
        repository.as_os_str(),
        OsStr::new("config"),
        OsStr::new("--local"),
        OsStr::new("--no-includes"),
        OsStr::new("--null"),
        OsStr::new("--get"),
        OsStr::new("remote.origin.url"),
    ])?;
    let mut expected_origin = url.as_bytes().to_vec();
    expected_origin.push(0);
    if origin != expected_origin {
        return Err(cache_invalid(
            entry_root,
            "Git origin does not match the exact source locator",
        ));
    }
    Ok(())
}

fn verify_local_git_config(
    entry_root: &Path,
    url: &str,
    bytes: &[u8],
) -> Result<(), SourceResolveError> {
    let mut seen = BTreeSet::new();
    for record in bytes
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let Some(separator) = record.iter().position(|byte| *byte == b'\n') else {
            return Err(cache_invalid(
                entry_root,
                "malformed local Git configuration",
            ));
        };
        let key = &record[..separator];
        let value = &record[separator + 1..];
        if !seen.insert(key.to_vec()) {
            return Err(cache_invalid(
                entry_root,
                "duplicate local Git configuration",
            ));
        }
        let allowed = match key {
            b"core.repositoryformatversion" => value == b"0",
            b"core.filemode" | b"core.ignorecase" | b"core.precomposeunicode" => {
                value == b"true" || value == b"false"
            }
            b"core.bare" => value == b"false",
            b"core.logallrefupdates" => value == b"true",
            b"remote.origin.url" => value == url.as_bytes(),
            b"remote.origin.fetch" => value == GIT_ORIGIN_FETCH.as_bytes(),
            _ => false,
        };
        if !allowed {
            return Err(cache_invalid(
                entry_root,
                "local Git configuration contains a non-resolver setting",
            ));
        }
    }
    for required in [
        b"core.repositoryformatversion".as_slice(),
        b"core.filemode".as_slice(),
        b"core.bare".as_slice(),
        b"core.logallrefupdates".as_slice(),
        b"remote.origin.url".as_slice(),
        b"remote.origin.fetch".as_slice(),
    ] {
        if !seen.contains(required) {
            return Err(cache_invalid(
                entry_root,
                "local Git configuration is missing a resolver-owned setting",
            ));
        }
    }
    Ok(())
}

fn invalidate_git_cache_entry(entry_root: &Path) {
    let metadata_path = entry_root.join(GIT_CACHE_METADATA);
    let _ = std::fs::remove_file(metadata_path);
}

fn git_cache_identity(url: &str, requested_rev: &str) -> String {
    let mut hasher = Sha256::new();
    hash_bytes(&mut hasher, GIT_CACHE_POLICY);
    hash_bytes(&mut hasher, url.as_bytes());
    hash_bytes(&mut hasher, requested_rev.as_bytes());
    format_sha256(&hasher.finalize())
}

fn git_cache_metadata(url: &str, requested_rev: &str) -> Vec<u8> {
    let mut metadata = Vec::new();
    metadata.extend_from_slice(GIT_CACHE_POLICY);
    append_framed_bytes(&mut metadata, url.as_bytes());
    append_framed_bytes(&mut metadata, requested_rev.as_bytes());
    metadata
}

fn append_framed_bytes(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    output.extend_from_slice(bytes);
}

fn cache_invalid(path: &Path, message: impl Into<String>) -> SourceResolveError {
    SourceResolveError::GitCacheInvalid {
        path: path.to_path_buf(),
        message: message.into(),
    }
}

fn require_real_directory(path: &Path, message: &str) -> Result<(), SourceResolveError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| io_error(path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(cache_invalid(path, message));
    }
    Ok(())
}

fn require_regular_file(path: &Path, message: &str) -> Result<(), SourceResolveError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| io_error(path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(cache_invalid(path, message));
    }
    Ok(())
}

struct CacheEntryLock {
    file: File,
}

impl CacheEntryLock {
    fn acquire(path: &Path) -> Result<Self, SourceResolveError> {
        if let Ok(metadata) = std::fs::symlink_metadata(path)
            && (metadata.file_type().is_symlink() || !metadata.is_file())
        {
            return Err(cache_invalid(path, "cache lock is not a regular file"));
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|error| io_error(path, error))?;
        file.lock().map_err(|error| io_error(path, error))?;
        require_regular_file(path, "cache lock was replaced while being acquired")?;
        Ok(Self { file })
    }
}

impl Drop for CacheEntryLock {
    fn drop(&mut self) {
        // Keep the inode in place: unlinking a lock file lets a waiter lock the old inode while a
        // newcomer locks a replacement. Closing this handle releases the advisory lock safely.
        let _ = self.file.unlock();
    }
}

struct PendingCacheEntry {
    root: PathBuf,
    published: bool,
}

impl PendingCacheEntry {
    fn create(cache_dir: &Path, cache_identity: &str) -> Result<Self, SourceResolveError> {
        for _ in 0..128 {
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let root = cache_dir.join(format!(
                ".git-{cache_identity}.stage-{}-{sequence}",
                std::process::id()
            ));
            match std::fs::create_dir(&root) {
                Ok(()) => {
                    return Ok(Self {
                        root,
                        published: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(io_error(&root, error)),
            }
        }
        Err(cache_invalid(
            cache_dir,
            "could not allocate a unique Git cache staging directory",
        ))
    }
}

impl Drop for PendingCacheEntry {
    fn drop(&mut self) {
        if !self.published {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

#[derive(Debug)]
struct SourceEntry {
    relative_bytes: Vec<u8>,
    kind: SourceEntryKind,
}

#[derive(Debug)]
enum SourceEntryKind {
    File { path: PathBuf },
    Symlink { target_bytes: Vec<u8> },
}

fn visit_directory(
    real_dir: &Path,
    logical_dir: PathBuf,
    depth: usize,
    root: &Path,
    limits: LocalSourceLimits,
    visited_dirs: &mut BTreeSet<PathBuf>,
    files: &mut Vec<SourceEntry>,
) -> Result<(), SourceResolveError> {
    if depth > limits.max_depth {
        return Err(SourceResolveError::TooDeep {
            path: real_dir.to_path_buf(),
            limit: limits.max_depth,
        });
    }
    let canonical_dir = real_dir
        .canonicalize()
        .map_err(|error| io_error(real_dir, error))?;
    if !canonical_dir.starts_with(root) {
        return Err(SourceResolveError::SymlinkEscapesRoot {
            link: real_dir.to_path_buf(),
            target: canonical_dir,
        });
    }
    if !visited_dirs.insert(canonical_dir) {
        return Ok(());
    }

    let mut entries = std::fs::read_dir(real_dir)
        .map_err(|error| io_error(real_dir, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io_error(real_dir, error))?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let name = entry.file_name();
        if name == ".git" {
            continue;
        }
        let real_path = entry.path();
        let logical_path = logical_dir.join(&name);
        let metadata =
            std::fs::symlink_metadata(&real_path).map_err(|error| io_error(&real_path, error))?;
        if metadata.file_type().is_symlink() {
            let raw_target = read_and_validate_symlink_target(root, &real_path)?;
            push_entry(
                files,
                logical_path,
                SourceEntryKind::Symlink {
                    target_bytes: raw_os_bytes(raw_target.as_os_str()),
                },
                limits,
            )?;
        } else if metadata.is_dir() {
            visit_directory(
                &real_path,
                logical_path,
                depth + 1,
                root,
                limits,
                visited_dirs,
                files,
            )?;
        } else if metadata.is_file() {
            push_entry(
                files,
                logical_path,
                SourceEntryKind::File { path: real_path },
                limits,
            )?;
        } else {
            return Err(SourceResolveError::UnsupportedFileType { path: real_path });
        }
    }
    Ok(())
}

fn read_and_validate_symlink_target(
    root: &Path,
    link: &Path,
) -> Result<PathBuf, SourceResolveError> {
    // V1 policy hashes link spelling, requires an existing canonical target inside this root, and
    // rejects targets under excluded `.git` metadata. Target contents are visited independently
    // through the ordinary tree walk rather than dereferenced through the link.
    let raw_target = std::fs::read_link(link).map_err(|error| io_error(link, error))?;
    let absolute_target = if raw_target.is_absolute() {
        raw_target.clone()
    } else {
        link.parent()
            .unwrap_or_else(|| Path::new("."))
            .join(&raw_target)
    };
    let target = absolute_target
        .canonicalize()
        .map_err(|error| io_error(&absolute_target, error))?;
    if !target.starts_with(root) {
        return Err(SourceResolveError::SymlinkEscapesRoot {
            link: link.to_path_buf(),
            target,
        });
    }
    let relative_target = target
        .strip_prefix(root)
        .expect("root containment was checked above");
    if relative_target
        .components()
        .any(|component| component.as_os_str() == ".git")
    {
        return Err(SourceResolveError::SymlinkTargetsExcludedMetadata {
            link: link.to_path_buf(),
            target,
        });
    }
    Ok(raw_target)
}

fn push_entry(
    files: &mut Vec<SourceEntry>,
    relative: PathBuf,
    kind: SourceEntryKind,
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    if files.len() >= limits.max_files {
        return Err(SourceResolveError::TooManyFiles {
            limit: limits.max_files,
        });
    }
    files.push(SourceEntry {
        relative_bytes: raw_os_bytes(relative.as_os_str()),
        kind,
    });
    Ok(())
}

fn read_file_bounded(
    path: &Path,
    remaining: u64,
    limit: u64,
) -> Result<(Vec<u8>, bool), SourceResolveError> {
    let mut file = std::fs::File::open(path).map_err(|error| io_error(path, error))?;
    let metadata = file.metadata().map_err(|error| io_error(path, error))?;
    if !metadata.is_file() {
        return Err(SourceResolveError::UnsupportedFileType {
            path: path.to_path_buf(),
        });
    }
    if metadata.len() > remaining {
        return Err(SourceResolveError::TooManyBytes { limit });
    }

    let initial_capacity =
        usize::try_from(metadata.len()).map_err(|_| SourceResolveError::TooManyBytes { limit })?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(initial_capacity)
        .map_err(|_| SourceResolveError::TooManyBytes { limit })?;
    let mut chunk = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut chunk)
            .map_err(|error| io_error(path, error))?;
        if count == 0 {
            break;
        }
        let next_len = (bytes.len() as u64)
            .checked_add(count as u64)
            .ok_or(SourceResolveError::TooManyBytes { limit })?;
        if next_len > remaining {
            return Err(SourceResolveError::TooManyBytes { limit });
        }
        bytes.extend_from_slice(&chunk[..count]);
    }

    Ok((bytes, is_executable(&metadata)))
}

#[cfg(unix)]
fn is_executable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &std::fs::Metadata) -> bool {
    false
}

fn raw_os_bytes(value: &OsStr) -> Vec<u8> {
    value.as_encoded_bytes().to_vec()
}

fn hash_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hash_length(hasher, bytes.len() as u64);
    hasher.update(bytes);
}

fn hash_length(hasher: &mut Sha256, length: u64) {
    hasher.update(length.to_le_bytes());
}

fn io_error(path: &Path, error: std::io::Error) -> SourceResolveError {
    SourceResolveError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

fn run_git<I, S>(args: I) -> Result<(), SourceResolveError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_git_output(args)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(SourceResolveError::Git {
            operation: "command".to_owned(),
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

fn run_git_stdout<I, S>(args: I) -> Result<String, SourceResolveError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_git_output(args)?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(SourceResolveError::Git {
            operation: "command".to_owned(),
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

fn run_git_bytes_stdout<I, S>(args: I) -> Result<Vec<u8>, SourceResolveError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_git_output(args)?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(SourceResolveError::Git {
            operation: "command".to_owned(),
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

fn run_git_output<I, S>(args: I) -> Result<std::process::Output, SourceResolveError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    sealed_git_command()
        .args(args)
        .output()
        .map_err(|error| SourceResolveError::Git {
            operation: "spawn".to_owned(),
            status: None,
            stderr: error.to_string(),
        })
}

fn sealed_git_command() -> Command {
    let mut command = Command::new("git");
    for (key, _) in std::env::vars_os() {
        let key_text = key.to_string_lossy();
        if key_text.starts_with("GIT_CONFIG_KEY_") || key_text.starts_with("GIT_CONFIG_VALUE_") {
            command.env_remove(key);
        }
    }
    for key in [
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_ASKPASS",
        "GIT_CEILING_DIRECTORIES",
        "GIT_COMMON_DIR",
        "GIT_CONFIG",
        "GIT_CONFIG_COUNT",
        "GIT_CONFIG_PARAMETERS",
        "GIT_DIFF_OPTS",
        "GIT_DIR",
        "GIT_EDITOR",
        "GIT_EXEC_PATH",
        "GIT_EXTERNAL_DIFF",
        "GIT_GRAFT_FILE",
        "GIT_INDEX_FILE",
        "GIT_NAMESPACE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_PAGER",
        "GIT_PROXY_COMMAND",
        "GIT_REPLACE_REF_BASE",
        "GIT_SEQUENCE_EDITOR",
        "GIT_SHALLOW_FILE",
        "GIT_SSH",
        "GIT_SSH_COMMAND",
        "GIT_WORK_TREE",
        "SSH_ASKPASS",
    ] {
        command.env_remove(key);
    }
    command
        .env("GIT_ALLOW_PROTOCOL", "file:https:http:ssh:git")
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null_device())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_LFS_SKIP_SMUDGE", "1")
        .env("GIT_PROTOCOL_FROM_USER", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .arg("--no-replace-objects")
        .arg("-c")
        .arg(format!("core.hooksPath={}", null_device()))
        .args([
            "-c",
            "protocol.allow=never",
            "-c",
            "protocol.file.allow=always",
            "-c",
            "protocol.http.allow=always",
            "-c",
            "protocol.https.allow=always",
            "-c",
            "protocol.ssh.allow=always",
            "-c",
            "protocol.git.allow=always",
            "-c",
            "protocol.ext.allow=never",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.autocrlf=false",
            "-c",
            "fetch.fsckObjects=true",
            "-c",
            "transfer.fsckObjects=true",
            "-c",
            "fetch.recurseSubmodules=false",
            "-c",
            "submodule.recurse=false",
            "-c",
            "filter.lfs.smudge=",
            "-c",
            "filter.lfs.required=false",
        ]);
    command
}

#[cfg(unix)]
fn null_device() -> &'static str {
    "/dev/null"
}

#[cfg(windows)]
fn null_device() -> &'static str {
    "NUL"
}

fn format_sha256(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::PackageName;
    use std::time::{SystemTime, UNIX_EPOCH};

    const PACKAGE_FIXTURES: &[&str] = &[
        "arithmetic-kernels",
        "generated-table",
        "file-journal",
        "network-overreach",
        "axiom-ledger",
        "provider-switchboard",
        "capability-vault",
        "graph-workbench",
    ];

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "omega-packages-{name}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn run_test_git<I, S>(directory: &Path, args: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = Command::new("git")
            .current_dir(directory)
            .args(args)
            .output()
            .expect("spawn git");
        assert!(
            output.status.success(),
            "git command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn create_git_source(name: &str) -> (PathBuf, String) {
        let root = temp_root(name);
        std::fs::create_dir_all(&root).expect("create git source");
        run_test_git(&root, ["init", "--quiet"]);
        run_test_git(&root, ["config", "user.email", "omega@example.invalid"]);
        run_test_git(&root, ["config", "user.name", "Omega Tests"]);
        std::fs::write(root.join("main.omg"), "machine Main::main() {}\n").expect("write source");
        run_test_git(&root, ["add", "main.omg"]);
        run_test_git(&root, ["commit", "--quiet", "-m", "initial"]);
        let output = Command::new("git")
            .current_dir(&root)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("read head");
        assert!(output.status.success());
        let commit = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        (root, commit)
    }

    fn package_fixtures_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../../../fixtures/packages")
    }

    fn git_cache_entry_root(cache: &Path, url: &str, requested_rev: &str) -> PathBuf {
        cache.join(format!("git-{}", git_cache_identity(url, requested_rev)))
    }

    #[test]
    fn package_fixtures_resolve_as_distinct_local_sources() {
        let fixtures_root = package_fixtures_root();
        let mut identities = BTreeSet::new();
        for package in PACKAGE_FIXTURES {
            PackageName::parse(*package).expect("fixture package names must be kebab-case");
            let root = fixtures_root.join(package);
            assert!(root.join("build.omg").is_file());
            assert!(root.join("main.omg").is_file());

            let resolved =
                resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve fixture");
            assert!(resolved.file_count >= 3);
            assert!(identities.insert(resolved.content_identity));
        }
        assert_eq!(identities.len(), PACKAGE_FIXTURES.len());
    }

    #[test]
    fn local_source_identity_is_order_independent_and_ignores_git_dir() {
        let root = temp_root("identity");
        std::fs::create_dir_all(root.join("src")).expect("create source tree");
        std::fs::create_dir_all(root.join(".git")).expect("create git dir");
        std::fs::write(root.join("src/lib.omg"), "machine Lib::id() {}\n").expect("write source");
        std::fs::write(root.join("README.md"), "package\n").expect("write readme");
        std::fs::write(root.join(".git/index"), "ignored").expect("write ignored git data");

        let first = resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve");
        std::fs::write(root.join(".git/index"), "ignored but changed")
            .expect("change ignored git data");
        let second = resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve");

        assert_eq!(first.file_count, 2);
        assert_eq!(first.content_identity, second.content_identity);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn local_source_identity_changes_when_source_bytes_change() {
        let root = temp_root("bytes");
        std::fs::create_dir_all(&root).expect("create source tree");
        std::fs::write(root.join("main.omg"), "machine Main::a() {}\n").expect("write source");
        let first = resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve");

        std::fs::write(root.join("main.omg"), "machine Main::b() {}\n").expect("rewrite source");
        let second = resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve");

        assert_ne!(first.content_identity, second.content_identity);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn local_source_path_encoding_preserves_non_utf8_bytes() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let first = OsString::from_vec(b"source-\x80.omg".to_vec());
        let second = OsString::from_vec(b"source-\x81.omg".to_vec());

        assert_eq!(raw_os_bytes(&first), b"source-\x80.omg");
        assert_eq!(raw_os_bytes(&second), b"source-\x81.omg");
        assert_ne!(raw_os_bytes(&first), raw_os_bytes(&second));
    }

    #[cfg(all(unix, not(target_vendor = "apple")))]
    #[test]
    fn local_source_identity_distinguishes_non_utf8_paths() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let first_root = temp_root("non-utf8-first");
        let second_root = temp_root("non-utf8-second");
        std::fs::create_dir_all(&first_root).expect("create first source tree");
        std::fs::create_dir_all(&second_root).expect("create second source tree");
        let first_name = OsString::from_vec(b"source-\x80.omg".to_vec());
        let second_name = OsString::from_vec(b"source-\x81.omg".to_vec());
        std::fs::write(first_root.join(first_name), "same bytes").expect("write first source");
        std::fs::write(second_root.join(second_name), "same bytes").expect("write second source");

        let first =
            resolve_local_source(&first_root, LocalSourceLimits::default()).expect("resolve first");
        let second = resolve_local_source(&second_root, LocalSourceLimits::default())
            .expect("resolve second");

        assert_ne!(first.content_identity, second.content_identity);

        let _ = std::fs::remove_dir_all(&first_root);
        let _ = std::fs::remove_dir_all(&second_root);
    }

    #[cfg(unix)]
    #[test]
    fn local_source_rejects_symlinks_into_excluded_git_metadata() {
        let root = temp_root("symlink-git-metadata");
        std::fs::create_dir_all(root.join(".git")).expect("create ignored target directory");
        let target = root.join(".git/target.omg");
        let link = root.join("linked.omg");
        std::fs::write(&target, "first target bytes").expect("write target");
        std::os::unix::fs::symlink(".git/target.omg", &link).expect("create symlink");

        let error = resolve_local_source(&root, LocalSourceLimits::default())
            .expect_err("excluded metadata target must reject");
        assert!(matches!(
            error,
            SourceResolveError::SymlinkTargetsExcludedMetadata { .. }
        ));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn local_source_identity_hashes_internal_symlink_spelling_and_reachable_target() {
        let root = temp_root("symlink-identity");
        std::fs::create_dir_all(&root).expect("create source tree");
        let target = root.join("target.omg");
        let link = root.join("linked.omg");
        std::fs::write(&target, "first target bytes").expect("write target");
        std::os::unix::fs::symlink("target.omg", &link).expect("create symlink");

        let first = resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve");
        std::fs::write(&target, "different target bytes").expect("rewrite target");
        let changed_target =
            resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve target");
        assert_ne!(first.content_identity, changed_target.content_identity);

        std::fs::remove_file(&link).expect("remove symlink");
        std::os::unix::fs::symlink("./target.omg", &link).expect("recreate symlink");
        let changed_spelling = resolve_local_source(&root, LocalSourceLimits::default())
            .expect("resolve spelling change");
        assert_ne!(
            changed_target.content_identity,
            changed_spelling.content_identity
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn local_source_identity_distinguishes_executable_mode() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_root("executable-mode");
        std::fs::create_dir_all(&root).expect("create source tree");
        let source = root.join("generate");
        std::fs::write(&source, "same bytes").expect("write source");
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o644))
            .expect("make source non-executable");
        let non_executable =
            resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve mode");

        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755))
            .expect("make source executable");
        let executable =
            resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve mode");

        assert_ne!(non_executable.content_identity, executable.content_identity);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn local_source_rejects_special_file_kind() {
        use std::os::unix::net::UnixListener;

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let root = PathBuf::from("/tmp").join(format!(
            "omega-source-socket-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create source tree");
        let socket_path = root.join("source.sock");
        let listener = UnixListener::bind(&socket_path).expect("create Unix socket");
        let expected_path = root
            .canonicalize()
            .expect("canonicalize source tree")
            .join("source.sock");

        let error = resolve_local_source(&root, LocalSourceLimits::default())
            .expect_err("special file should reject");

        assert_eq!(
            error,
            SourceResolveError::UnsupportedFileType {
                path: expected_path
            }
        );

        drop(listener);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn local_source_limits_reject_too_many_files() {
        let root = temp_root("files");
        std::fs::create_dir_all(&root).expect("create source tree");
        std::fs::write(root.join("a.omg"), "").expect("write source");

        let error = resolve_local_source(
            &root,
            LocalSourceLimits {
                max_files: 0,
                ..LocalSourceLimits::default()
            },
        )
        .expect_err("file limit should reject");

        assert_eq!(error, SourceResolveError::TooManyFiles { limit: 0 });

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn local_source_limits_reject_file_before_reading_past_byte_limit() {
        let root = temp_root("bytes-limit");
        std::fs::create_dir_all(&root).expect("create source tree");
        std::fs::write(root.join("source.omg"), "four").expect("write source");

        let error = resolve_local_source(
            &root,
            LocalSourceLimits {
                max_bytes: 3,
                ..LocalSourceLimits::default()
            },
        )
        .expect_err("byte limit should reject");

        assert_eq!(error, SourceResolveError::TooManyBytes { limit: 3 });

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn local_source_rejects_symlink_escape() {
        let root = temp_root("symlink");
        let outside = temp_root("outside");
        std::fs::create_dir_all(&root).expect("create source tree");
        std::fs::create_dir_all(&outside).expect("create outside tree");
        std::fs::write(outside.join("secret.omg"), "secret").expect("write outside source");
        std::os::unix::fs::symlink(outside.join("secret.omg"), root.join("secret.omg"))
            .expect("create escaping symlink");

        let error =
            resolve_local_source(&root, LocalSourceLimits::default()).expect_err("escape rejects");

        assert!(matches!(
            error,
            SourceResolveError::SymlinkEscapesRoot { .. }
        ));

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn git_source_resolves_exact_commit_and_local_identity() {
        let (repo, commit) = create_git_source("git");
        let cache = temp_root("git-cache");

        let resolved = resolve_git_source(
            &GitSourceSpec {
                url: repo.display().to_string(),
                rev: Some(commit.clone()),
            },
            &cache,
            LocalSourceLimits::default(),
        )
        .expect("resolve git source");

        assert_eq!(resolved.commit, commit);
        assert_eq!(resolved.local.file_count, 1);
        assert!(!resolved.tree.is_empty());

        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_cache_does_not_admit_untracked_checkout_injection() {
        let (repo, _) = create_git_source("git-untracked-source");
        let cache = temp_root("git-untracked-cache");
        let spec = GitSourceSpec {
            url: repo.display().to_string(),
            rev: Some("HEAD".to_owned()),
        };
        let first =
            resolve_git_source(&spec, &cache, LocalSourceLimits::default()).expect("prime cache");
        let injected = first.checkout_root.join("injected.omg");
        std::fs::write(&injected, "machine Injected::main() {}\n")
            .expect("inject untracked source");

        let second = resolve_git_source(&spec, &cache, LocalSourceLimits::default())
            .expect("resolve clean checkout");

        assert!(!injected.exists());
        assert_eq!(second.local.file_count, 1);
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_cache_identity_is_full_policy_versioned_and_injectively_framed() {
        let first = git_cache_identity("a\0b", "c");
        let second = git_cache_identity("a", "b\0c");

        assert_eq!(first.len(), 64);
        assert!(first.chars().all(|character| character.is_ascii_hexdigit()));
        assert_ne!(first, second);
        assert_ne!(first, git_cache_identity("a\0b", "C"));
    }

    #[test]
    fn git_cache_serializes_access_without_unlinking_its_lock() {
        use std::sync::mpsc;
        use std::time::Duration;

        let cache = temp_root("git-lock");
        std::fs::create_dir_all(&cache).expect("create cache");
        let lock_path = cache.join("entry.lock");
        let first = CacheEntryLock::acquire(&lock_path).expect("acquire first lock");
        let thread_lock_path = lock_path.clone();
        let (sender, receiver) = mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let second =
                CacheEntryLock::acquire(&thread_lock_path).expect("acquire serialized lock");
            sender.send(()).expect("report lock acquisition");
            drop(second);
        });

        assert!(receiver.recv_timeout(Duration::from_millis(100)).is_err());
        drop(first);
        receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("waiter should acquire released lock");
        waiter.join().expect("join lock waiter");
        assert!(lock_path.is_file());

        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_cache_rejects_resolver_metadata_substitution() {
        let (repo, _) = create_git_source("git-metadata-source");
        let (substitute, _) = create_git_source("git-metadata-substitute");
        let cache = temp_root("git-metadata-cache");
        let url = repo.display().to_string();
        let substitute_url = substitute.display().to_string();
        let spec = GitSourceSpec {
            url: url.clone(),
            rev: Some("HEAD".to_owned()),
        };
        resolve_git_source(&spec, &cache, LocalSourceLimits::default()).expect("prime cache");
        let entry = git_cache_entry_root(&cache, &url, "HEAD");
        std::fs::write(
            entry.join(GIT_CACHE_METADATA),
            git_cache_metadata(&substitute_url, "HEAD"),
        )
        .expect("substitute metadata");

        let error = resolve_git_source(&spec, &cache, LocalSourceLimits::default())
            .expect_err("substituted metadata must reject");

        assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
        assert!(!entry.join(GIT_CACHE_METADATA).exists());
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&substitute);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_cache_rejects_origin_substitution() {
        let (repo, _) = create_git_source("git-origin-source");
        let (substitute, _) = create_git_source("git-origin-substitute");
        let cache = temp_root("git-origin-cache");
        let url = repo.display().to_string();
        let substitute_url = substitute.display().to_string();
        let spec = GitSourceSpec {
            url: url.clone(),
            rev: Some("HEAD".to_owned()),
        };
        let resolved =
            resolve_git_source(&spec, &cache, LocalSourceLimits::default()).expect("prime cache");
        run_test_git(
            &resolved.checkout_root,
            ["remote", "set-url", "origin", substitute_url.as_str()],
        );
        let entry = git_cache_entry_root(&cache, &url, "HEAD");

        let error = resolve_git_source(&spec, &cache, LocalSourceLimits::default())
            .expect_err("substituted origin must reject");

        assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
        assert!(!entry.join(GIT_CACHE_METADATA).exists());
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&substitute);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_cache_rejects_local_filter_configuration_before_checkout() {
        let (repo, _) = create_git_source("git-filter-source");
        std::fs::write(repo.join(".gitattributes"), "*.omg filter=omega-test\n")
            .expect("write attributes");
        run_test_git(&repo, ["add", ".gitattributes"]);
        run_test_git(&repo, ["commit", "--quiet", "-m", "declare filter"]);
        let cache = temp_root("git-filter-cache");
        let sentinel = cache.join("filter-ran");
        let url = repo.display().to_string();
        let spec = GitSourceSpec {
            url,
            rev: Some("HEAD".to_owned()),
        };
        let resolved =
            resolve_git_source(&spec, &cache, LocalSourceLimits::default()).expect("prime cache");
        run_test_git(
            &resolved.checkout_root,
            [
                "config",
                "--local",
                "filter.omega-test.smudge",
                &format!("touch {}", sentinel.display()),
            ],
        );

        let error = resolve_git_source(&spec, &cache, LocalSourceLimits::default())
            .expect_err("local filter configuration must reject");

        assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
        assert!(!sentinel.exists());
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_commands_seal_ambient_config_protocol_and_execution_injection() {
        let command = sealed_git_command();
        let environment = command
            .get_envs()
            .map(|(key, value)| (key.to_owned(), value.map(OsStr::to_owned)))
            .collect::<std::collections::BTreeMap<_, _>>();
        let arguments = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            environment.get(OsStr::new("GIT_CONFIG_GLOBAL")),
            Some(&Some(OsStr::new(null_device()).to_owned()))
        );
        assert_eq!(
            environment.get(OsStr::new("GIT_CONFIG_NOSYSTEM")),
            Some(&Some(OsStr::new("1").to_owned()))
        );
        for removed in [
            "GIT_CONFIG_COUNT",
            "GIT_CONFIG_PARAMETERS",
            "GIT_EXEC_PATH",
            "GIT_SSH_COMMAND",
            "GIT_EXTERNAL_DIFF",
        ] {
            assert_eq!(environment.get(OsStr::new(removed)), Some(&None));
        }
        assert!(
            arguments
                .iter()
                .any(|argument| argument == "--no-replace-objects")
        );
        assert!(
            arguments
                .iter()
                .any(|argument| argument == "protocol.allow=never")
        );
        assert!(
            arguments
                .iter()
                .any(|argument| argument == "protocol.ext.allow=never")
        );
        assert!(
            arguments
                .iter()
                .any(|argument| argument == "fetch.recurseSubmodules=false")
        );
    }

    #[test]
    fn git_source_rejects_submodule_manifest() {
        let (repo, commit) = create_git_source("git-submodule");
        let cache = temp_root("git-submodule-cache");
        let url = repo.display().to_string();
        let spec = GitSourceSpec {
            url,
            rev: Some("HEAD".to_owned()),
        };
        let initial = resolve_git_source(&spec, &cache, LocalSourceLimits::default())
            .expect("resolve initial source");
        let checkout_source = initial.checkout_root.join("main.omg");
        let initial_checkout = std::fs::read(&checkout_source).expect("read initial checkout");

        std::fs::write(repo.join(".gitmodules"), "[submodule \"dep\"]\n")
            .expect("write gitmodules");
        std::fs::write(repo.join("main.omg"), "machine Main::changed() {}\n")
            .expect("change source");
        run_test_git(&repo, ["add", ".gitmodules"]);
        run_test_git(&repo, ["add", "main.omg"]);
        run_test_git(&repo, ["commit", "--quiet", "-m", "submodule manifest"]);

        let error = resolve_git_source(&spec, &cache, LocalSourceLimits::default())
            .expect_err("submodule manifest should reject");

        assert!(matches!(
            error,
            SourceResolveError::GitSubmodulesUnsupported { .. }
        ));
        assert_eq!(
            std::fs::read(&checkout_source).expect("read checkout after rejection"),
            initial_checkout,
            "the fetched submodule tree must be rejected before checkout"
        );
        assert!(!initial.checkout_root.join("../source.identity").exists());
        assert!(!commit.is_empty());
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&cache);
    }
}
