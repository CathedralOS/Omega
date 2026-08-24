use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    let checkout_root = cache_dir.join(format!(
        "git-{}",
        short_hash(format!("{}\0{}", spec.url, requested_rev).as_bytes())
    ));

    if !checkout_root.exists() {
        run_git([
            OsStr::new("clone"),
            OsStr::new("--no-checkout"),
            OsStr::new("--quiet"),
            OsStr::new("--"),
            OsStr::new(&spec.url),
            checkout_root.as_os_str(),
        ])?;
    }

    run_git([
        OsStr::new("-C"),
        checkout_root.as_os_str(),
        OsStr::new("fetch"),
        OsStr::new("--quiet"),
        OsStr::new("origin"),
        OsStr::new(&requested_rev),
    ])?;
    let commit = run_git_stdout([
        OsStr::new("-C"),
        checkout_root.as_os_str(),
        OsStr::new("rev-parse"),
        OsStr::new("--verify"),
        OsStr::new("FETCH_HEAD^{commit}"),
    ])?;
    let commit = commit.trim().to_owned();

    run_git([
        OsStr::new("-C"),
        checkout_root.as_os_str(),
        OsStr::new("checkout"),
        OsStr::new("--quiet"),
        OsStr::new("--detach"),
        OsStr::new(&commit),
    ])?;
    let tree = run_git_stdout([
        OsStr::new("-C"),
        checkout_root.as_os_str(),
        OsStr::new("rev-parse"),
        OsStr::new("--verify"),
        OsStr::new("HEAD^{tree}"),
    ])?;
    let tree = tree.trim().to_owned();

    let gitmodules = checkout_root.join(".gitmodules");
    if gitmodules.exists() {
        return Err(SourceResolveError::GitSubmodulesUnsupported { path: gitmodules });
    }

    let local = resolve_local_source(&checkout_root, limits)?;
    Ok(ResolvedGitSource {
        url: spec.url.clone(),
        requested_rev,
        commit,
        tree,
        checkout_root,
        local,
    })
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
    if target.starts_with(root) {
        Ok(raw_target)
    } else {
        Err(SourceResolveError::SymlinkEscapesRoot {
            link: link.to_path_buf(),
            target,
        })
    }
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
    let output =
        Command::new("git")
            .args(args)
            .output()
            .map_err(|error| SourceResolveError::Git {
                operation: "spawn".to_owned(),
                status: None,
                stderr: error.to_string(),
            })?;
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
    let output =
        Command::new("git")
            .args(args)
            .output()
            .map_err(|error| SourceResolveError::Git {
                operation: "spawn".to_owned(),
                status: None,
                stderr: error.to_string(),
            })?;
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

fn short_hash(bytes: &[u8]) -> String {
    format_sha256(&Sha256::digest(bytes))[..16].to_owned()
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
    fn local_source_identity_hashes_symlink_spelling_without_following_it() {
        let root = temp_root("symlink-identity");
        std::fs::create_dir_all(root.join(".git")).expect("create ignored target directory");
        let target = root.join(".git/target.omg");
        let link = root.join("linked.omg");
        std::fs::write(&target, "first target bytes").expect("write target");
        std::os::unix::fs::symlink(".git/target.omg", &link).expect("create symlink");

        let first =
            resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve first");
        std::fs::write(&target, "different target bytes").expect("rewrite ignored target");
        let changed_target = resolve_local_source(&root, LocalSourceLimits::default())
            .expect("resolve target change");
        assert_eq!(first.content_identity, changed_target.content_identity);

        std::fs::remove_file(&link).expect("remove symlink");
        std::os::unix::fs::symlink("./.git/target.omg", &link).expect("recreate symlink");
        let changed_spelling = resolve_local_source(&root, LocalSourceLimits::default())
            .expect("resolve spelling change");
        assert_ne!(first.content_identity, changed_spelling.content_identity);

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
    fn git_source_rejects_submodule_manifest() {
        let (repo, commit) = create_git_source("git-submodule");
        std::fs::write(repo.join(".gitmodules"), "[submodule \"dep\"]\n")
            .expect("write gitmodules");
        run_test_git(&repo, ["add", ".gitmodules"]);
        run_test_git(&repo, ["commit", "--quiet", "-m", "submodule manifest"]);
        let cache = temp_root("git-submodule-cache");

        let error = resolve_git_source(
            &GitSourceSpec {
                url: repo.display().to_string(),
                rev: Some("HEAD".to_owned()),
            },
            &cache,
            LocalSourceLimits::default(),
        )
        .expect_err("submodule manifest should reject");

        assert!(matches!(
            error,
            SourceResolveError::GitSubmodulesUnsupported { .. }
        ));

        assert!(!commit.is_empty());
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&cache);
    }
}
