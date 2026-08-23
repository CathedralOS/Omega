use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

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
pub enum SourceResolveError {
    Io { path: PathBuf, message: String },
    NotDirectory { path: PathBuf },
    TooManyFiles { limit: usize },
    TooManyBytes { limit: u64 },
    TooDeep { path: PathBuf, limit: usize },
    SymlinkEscapesRoot { link: PathBuf, target: PathBuf },
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

    let mut files = Vec::new();
    let mut visited_dirs = BTreeSet::new();
    visit_directory(
        &root,
        PathBuf::new(),
        0,
        &root,
        limits,
        &mut visited_dirs,
        &mut files,
    )?;
    files.sort_by(|left, right| left.relative.cmp(&right.relative));

    let mut hasher = Sha256::new();
    hasher.update(b"omega-local-source-v1\0");
    let mut byte_count = 0_u64;
    for file in &files {
        byte_count = byte_count.checked_add(file.bytes.len() as u64).ok_or(
            SourceResolveError::TooManyBytes {
                limit: limits.max_bytes,
            },
        )?;
        if byte_count > limits.max_bytes {
            return Err(SourceResolveError::TooManyBytes {
                limit: limits.max_bytes,
            });
        }
        hasher.update(b"file\0");
        hasher.update(path_bytes(&file.relative).as_bytes());
        hasher.update(b"\0");
        hasher.update((file.bytes.len() as u64).to_le_bytes());
        hasher.update(b"\0");
        hasher.update(&file.bytes);
        hasher.update(b"\0");
    }

    Ok(ResolvedLocalSource {
        root,
        file_count: files.len(),
        byte_count,
        content_identity: format_sha256(&hasher.finalize()),
    })
}

#[derive(Debug)]
struct SourceFile {
    relative: PathBuf,
    bytes: Vec<u8>,
}

fn visit_directory(
    real_dir: &Path,
    logical_dir: PathBuf,
    depth: usize,
    root: &Path,
    limits: LocalSourceLimits,
    visited_dirs: &mut BTreeSet<PathBuf>,
    files: &mut Vec<SourceFile>,
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
            let target = resolve_symlink_target(root, &real_path)?;
            let target_metadata =
                std::fs::metadata(&target).map_err(|error| io_error(&target, error))?;
            if target_metadata.is_dir() {
                visit_directory(
                    &target,
                    logical_path,
                    depth + 1,
                    root,
                    limits,
                    visited_dirs,
                    files,
                )?;
            } else if target_metadata.is_file() {
                push_file(files, logical_path, &target, limits)?;
            }
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
            push_file(files, logical_path, &real_path, limits)?;
        }
    }
    Ok(())
}

fn resolve_symlink_target(root: &Path, link: &Path) -> Result<PathBuf, SourceResolveError> {
    let raw_target = std::fs::read_link(link).map_err(|error| io_error(link, error))?;
    let absolute_target = if raw_target.is_absolute() {
        raw_target
    } else {
        link.parent()
            .unwrap_or_else(|| Path::new("."))
            .join(raw_target)
    };
    let target = absolute_target
        .canonicalize()
        .map_err(|error| io_error(&absolute_target, error))?;
    if target.starts_with(root) {
        Ok(target)
    } else {
        Err(SourceResolveError::SymlinkEscapesRoot {
            link: link.to_path_buf(),
            target,
        })
    }
}

fn push_file(
    files: &mut Vec<SourceFile>,
    relative: PathBuf,
    path: &Path,
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    if files.len() >= limits.max_files {
        return Err(SourceResolveError::TooManyFiles {
            limit: limits.max_files,
        });
    }
    let bytes = std::fs::read(path).map_err(|error| io_error(path, error))?;
    files.push(SourceFile { relative, bytes });
    Ok(())
}

fn path_bytes(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn io_error(path: &Path, error: std::io::Error) -> SourceResolveError {
    SourceResolveError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
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
    use std::time::{SystemTime, UNIX_EPOCH};

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
}
