//! Shared fixture construction for owner-local package-source tests.

use crate::error::SourceResolveError;
use crate::git::cache::identity::git_cache_identity;
use crate::git::cache::repository::VerifiedGitRepository;
use crate::git::executable::executor::test_system_git_executor;
use crate::git::request::GitSourceRequest;
use crate::git::resolution::resolve_git_source_with_storage;
use crate::limits::LocalSourceLimits;
use crate::local::model::ResolvedLocalSnapshot;
use crate::local::operations::resolve_local_source_snapshot_with_storage;
use crate::observations::resolved::ResolvedGitSource;
use crate::storage::SourceResolverStorage;
use crate::tree::filesystem::open_absolute_directory_nofollow;
use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const PACKAGE_FIXTURES: &[&str] = &[
    "arithmetic-kernels",
    "generated-table",
    "file-journal",
    "network-overreach",
    "remote-journal",
    "axiom-ledger",
    "opaque-carrier",
    "provider-switchboard",
    "capability-vault",
    "graph-workbench",
];

pub(crate) fn temp_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let temporary_directory = std::env::temp_dir()
        .canonicalize()
        .expect("canonicalize test temporary directory");
    temporary_directory.join(format!(
        "omega-package-source-{name}-{}-{stamp}",
        std::process::id()
    ))
}

pub(crate) fn resolve_git_source(
    request: &GitSourceRequest,
    hardened_base: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    let primary_git = test_system_git_executor(request.execution_transport())?
        .execution_backend
        .executable()
        .to_path_buf();
    let storage =
        SourceResolverStorage::for_hardened_base_with_primary_git(hardened_base, primary_git)?;
    resolve_git_source_with_storage(request, &storage, limits)
}

pub(crate) fn resolve_local_source_snapshot(
    root: impl AsRef<Path>,
    hardened_base: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSnapshot, SourceResolveError> {
    let storage = SourceResolverStorage::for_hardened_base(hardened_base)?;
    resolve_local_source_snapshot_with_storage(root, &storage, limits)
}

pub(crate) fn git_storage_lane(hardened_base: &Path) -> PathBuf {
    resolver_storage_root(hardened_base).join("git-sources")
}

#[cfg(unix)]
pub(crate) fn external_local_storage_lane(hardened_base: &Path) -> PathBuf {
    resolver_storage_root(hardened_base).join("external-local-sources")
}

fn resolver_storage_root(hardened_base: &Path) -> PathBuf {
    hardened_base
        .join("CathedralOS")
        .join("Omega")
        .join("source")
        .join("v1")
}

#[cfg(target_os = "macos")]
pub(crate) fn change_macos_acl(path: &Path, arguments: &[&str]) {
    let status = Command::new("/bin/chmod")
        .args(arguments)
        .arg(path)
        .status()
        .expect("run the concrete macOS ACL editor");
    assert!(
        status.success(),
        "macOS ACL edit failed for {}",
        path.display()
    );
}

pub(crate) fn local_git_request(repository: &Path, revision: &str) -> GitSourceRequest {
    GitSourceRequest::for_local_test_repository(repository, Some(revision.to_owned()))
        .expect("local Git fixture request")
}
pub(crate) fn run_test_git<I, S>(directory: &Path, args: I)
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

pub(crate) fn run_test_git_with_input<I, S>(directory: &Path, args: I, input: &[u8]) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new("git")
        .current_dir(directory)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn git");
    command
        .stdin
        .take()
        .expect("capture git stdin")
        .write_all(input)
        .expect("write git input");
    let output = command.wait_with_output().expect("wait for git");
    assert!(
        output.status.success(),
        "git command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("Git test output is UTF-8")
        .trim()
        .to_owned()
}

pub(crate) fn create_git_source(name: &str) -> (PathBuf, String) {
    create_git_source_with_format(name, None)
}

pub(crate) fn create_git_source_with_format(
    name: &str,
    object_format: Option<&str>,
) -> (PathBuf, String) {
    let root = temp_root(name);
    std::fs::create_dir_all(&root).expect("create git source");
    let mut init_arguments = vec!["init", "--quiet"];
    let object_format_argument = object_format.map(|format| format!("--object-format={format}"));
    if let Some(argument) = object_format_argument.as_deref() {
        init_arguments.push(argument);
    }
    run_test_git(&root, init_arguments);
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

pub(crate) fn add_empty_tree_commit(repository: &Path) -> String {
    let empty_tree =
        run_test_git_with_input(repository, ["hash-object", "-t", "tree", "--stdin"], b"");
    let main_blob = run_test_git_with_input(repository, ["rev-parse", "HEAD:main.omg"], b"");
    let root_tree_input =
        format!("100644 blob {main_blob}\tmain.omg\n040000 tree {empty_tree}\tempty\n");
    let root_tree = run_test_git_with_input(repository, ["mktree"], root_tree_input.as_bytes());
    let commit = run_test_git_with_input(
        repository,
        [
            "commit-tree",
            &root_tree,
            "-p",
            "HEAD",
            "-m",
            "add empty tree",
        ],
        b"",
    );
    run_test_git(repository, ["update-ref", "HEAD", &commit]);
    commit
}

pub(crate) fn package_fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .map(|ancestor| ancestor.join("tests/fixtures/packages"))
        .find(|fixtures| fixtures.is_dir())
        .expect("omega-package-source should live beneath the Omega workspace")
}

pub(crate) fn git_cache_entry_root(cache: &Path, request: &GitSourceRequest) -> PathBuf {
    git_storage_lane(cache).join(git_cache_entry_name(request))
}

fn git_cache_entry_name(request: &GitSourceRequest) -> String {
    format!(
        "git-{}",
        git_cache_identity(
            request.locator_identity(),
            request.requested_revision(),
            request.execution_transport(),
        )
    )
}

pub(crate) fn open_verified_git_repository(
    cache: &Path,
    request: &GitSourceRequest,
) -> VerifiedGitRepository {
    let canonical_cache = git_storage_lane(cache)
        .canonicalize()
        .expect("canonicalize Git cache");
    let cache_directory =
        open_absolute_directory_nofollow(&canonical_cache).expect("retain Git cache parent");
    let entry_root = canonical_cache.join(git_cache_entry_name(request));
    let entry_name = entry_root.file_name().expect("Git cache entry has a name");
    VerifiedGitRepository::open(
        &cache_directory,
        entry_name,
        &entry_root,
        request.locator_identity(),
        request.requested_revision(),
        request.execution_transport(),
        LocalSourceLimits::default(),
    )
    .expect("retain verified Git repository")
}

pub(crate) fn first_regular_descendant(root: &Path) -> PathBuf {
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("read test directory") {
            let path = entry.expect("read test entry").path();
            let metadata = std::fs::symlink_metadata(&path).expect("classify test entry");
            if metadata.is_file() {
                return path;
            }
            if metadata.is_dir() {
                pending.push(path);
            }
        }
    }
    panic!(
        "test tree contains no regular file beneath {}",
        root.display()
    );
}

#[cfg(unix)]
pub(crate) fn shell_command(script: &str) -> omega_resolver_execution::ResolverPreparedExecution {
    use omega_resolver_execution::{ResolverExecutionBackend, ResolverExecutionPhase};

    let temporary_root = std::env::temp_dir()
        .canonicalize()
        .expect("canonicalize test temporary directory");
    let shell = Path::new("/bin/sh")
        .canonicalize()
        .expect("canonicalize test shell");
    let backend = ResolverExecutionBackend::open(&shell, &[] as &[PathBuf])
        .expect("open test resolver backend");
    let mut command = backend
        .prepare(
            ResolverExecutionPhase::Fetch,
            Some(temporary_root.as_path()),
        )
        .expect("prepare bounded test shell");
    command.args(["-c", script]);
    command
}
