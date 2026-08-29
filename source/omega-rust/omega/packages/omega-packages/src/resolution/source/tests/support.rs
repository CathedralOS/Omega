use super::*;

pub(super) const PACKAGE_FIXTURES: &[&str] = &[
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

pub(super) fn temp_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let temporary_directory = std::env::temp_dir()
        .canonicalize()
        .expect("canonicalize test temporary directory");
    temporary_directory.join(format!(
        "omega-packages-{name}-{}-{stamp}",
        std::process::id()
    ))
}

#[cfg(target_os = "macos")]
pub(super) fn change_macos_acl(path: &Path, arguments: &[&str]) {
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

pub(super) fn local_git_request(repository: &Path, revision: &str) -> GitSourceRequest {
    GitSourceRequest::for_local_test_repository(repository, Some(revision.to_owned()))
        .expect("local Git fixture request")
}
pub(super) fn run_test_git<I, S>(directory: &Path, args: I)
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

pub(super) fn run_test_git_with_input<I, S>(directory: &Path, args: I, input: &[u8]) -> String
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

pub(super) fn create_git_source(name: &str) -> (PathBuf, String) {
    create_git_source_with_format(name, None)
}

pub(super) fn create_git_source_with_format(
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

pub(super) fn add_empty_tree_commit(repository: &Path) -> String {
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

pub(super) fn package_fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../../tests/fixtures/packages")
}

pub(super) fn git_cache_entry_root(cache: &Path, request: &GitSourceRequest) -> PathBuf {
    cache.join(format!(
        "git-{}",
        git_cache_identity(
            request.locator_identity(),
            request.requested_revision(),
            request.execution_transport(),
        )
    ))
}

pub(super) fn open_verified_git_repository(
    cache: &Path,
    request: &GitSourceRequest,
) -> VerifiedGitRepository {
    let canonical_cache = cache.canonicalize().expect("canonicalize Git cache");
    let cache_directory =
        open_absolute_directory_nofollow(&canonical_cache).expect("retain Git cache parent");
    let entry_root = git_cache_entry_root(&canonical_cache, request);
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

pub(super) fn first_regular_descendant(root: &Path) -> PathBuf {
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
pub(super) fn shell_command(script: &str) -> Command {
    let mut command = Command::new("sh");
    command.args(["-c", script]);
    command
}
