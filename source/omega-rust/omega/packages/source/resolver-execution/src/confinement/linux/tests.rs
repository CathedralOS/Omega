use super::*;
use crate::{
    ResolverExecutionBackend, ResolverExecutionGuarantee, ResolverExecutionGuaranteeDisposition,
    ResolverExecutionPhase,
};
use std::fs::{self, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn landlock_observation_does_not_overclaim_partial_write_or_execution_controls() {
    let Some(backend) = landlock_backend() else {
        return;
    };
    let root = temporary_directory("observation");
    let shell = canonical_executable("/bin/sh");
    let (_command, policy) = backend
        .command_with_observation(
            &shell,
            &[],
            ResolverExecutionPhase::RepositoryInitialization,
            None,
            Some(&root),
        )
        .expect("prepare Linux Landlock observation");

    assert_eq!(
        disposition(
            &policy,
            ResolverExecutionGuarantee::FilesystemWritesConfined
        ),
        ResolverExecutionGuaranteeDisposition::Unavailable
    );
    assert_eq!(
        disposition(&policy, ResolverExecutionGuarantee::ExecutablePathsConfined),
        ResolverExecutionGuaranteeDisposition::Unavailable
    );
    for unavailable in [
        ResolverExecutionGuarantee::FilesystemReadsConfined,
        ResolverExecutionGuarantee::NetworkDenied,
        ResolverExecutionGuarantee::NetworkEndpointsConfined,
    ] {
        assert_eq!(
            disposition(&policy, unavailable),
            ResolverExecutionGuaranteeDisposition::Unavailable
        );
    }
    fs::remove_dir_all(root).expect("remove observation root");
}

#[test]
fn landlock_allows_mutable_root_and_denies_sibling_writes() {
    let Some(backend) = landlock_backend() else {
        return;
    };
    let parent = temporary_directory("writes");
    let mutable = parent.join("mutable");
    let outside = parent.join("outside");
    fs::create_dir(&mutable).expect("create mutable root");
    fs::create_dir(&outside).expect("create denied sibling");
    let shell = canonical_executable("/bin/sh");
    let (mut command, policy) = backend
        .command_with_observation(
            &shell,
            &[],
            ResolverExecutionPhase::RepositoryInitialization,
            None,
            Some(&mutable),
        )
        .expect("prepare Linux write canary");
    command
        .args([
            "-c",
            r#"printf allowed > "$1/allowed" || exit 80; if printf denied > "$2/denied"; then exit 81; fi"#,
            "omega-resolver-linux-test",
        ])
        .arg(&mutable)
        .arg(&outside)
        .env_clear()
        .current_dir(&mutable)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let status = spawn(command, &policy)
        .expect("spawn Linux write canary")
        .wait()
        .expect("wait for Linux write canary");
    assert!(status.success());
    assert!(mutable.join("allowed").is_file());
    assert!(!outside.join("denied").exists());
    fs::remove_dir_all(parent).expect("remove write canary root");
}

#[test]
fn landlock_executes_only_the_exact_allowlist() {
    let Some(backend) = landlock_backend() else {
        return;
    };
    let root = temporary_directory("execute");
    let shell = canonical_executable("/bin/sh");
    let target = canonical_executable("/usr/bin/true");

    let denied = run_executable_canary(&backend, &root, &shell, &target, &[]);
    assert!(!denied.success(), "unlisted executable unexpectedly ran");

    let allowed = run_executable_canary(&backend, &root, &shell, &target, &[target.clone()]);
    assert!(allowed.success(), "listed executable was denied");
    fs::remove_dir_all(root).expect("remove executable canary root");
}

#[test]
fn landlock_closes_ambient_writable_descriptors_before_exec() {
    let Some(backend) = landlock_backend() else {
        return;
    };
    let parent = temporary_directory("descriptors");
    let mutable = parent.join("mutable");
    let outside = parent.join("outside");
    fs::create_dir(&mutable).expect("create mutable root");
    fs::create_dir(&outside).expect("create denied sibling");
    let outside_file = outside.join("ambient-write");
    let file = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(&outside_file)
        .expect("open ambient writable descriptor");
    let inherited_descriptor = file.as_raw_fd();
    let shell = canonical_executable("/bin/sh");
    let (mut command, policy) = backend
        .command_with_observation(
            &shell,
            &[],
            ResolverExecutionPhase::RepositoryInitialization,
            None,
            Some(&mutable),
        )
        .expect("prepare Linux descriptor canary");
    command
        .args(["-c", "printf denied >&9"])
        .env_clear()
        .current_dir(&mutable)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(inherited_descriptor, 9) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }

    let status = spawn(command, &policy)
        .expect("spawn Linux descriptor canary")
        .wait()
        .expect("wait for Linux descriptor canary");
    assert!(
        !status.success(),
        "ambient writable descriptor survived exec"
    );
    drop(file);
    assert!(
        fs::read(&outside_file)
            .expect("read denied file")
            .is_empty()
    );
    fs::remove_dir_all(parent).expect("remove descriptor canary root");
}

fn run_executable_canary(
    backend: &ResolverExecutionBackend,
    root: &Path,
    shell: &Path,
    target: &Path,
    additional_executables: &[PathBuf],
) -> std::process::ExitStatus {
    let (mut command, policy) = backend
        .command_with_inspection_read_root_observation(shell, additional_executables, root)
        .expect("prepare Linux executable canary");
    command
        .args(["-c", r#""$1""#, "omega-resolver-linux-test"])
        .arg(target)
        .env_clear()
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    spawn(command, &policy)
        .expect("spawn Linux executable canary")
        .wait()
        .expect("wait for Linux executable canary")
}

fn landlock_backend() -> Option<ResolverExecutionBackend> {
    if !backend_available() {
        return None;
    }
    let backend = ResolverExecutionBackend::open().expect("open Linux resolver backend");
    matches!(
        backend.identity(),
        ResolverExecutionBackendIdentity::LinuxLandlockV5
    )
    .then_some(backend)
}

fn disposition(
    policy: &ResolverExecutionPolicyObservation,
    guarantee: ResolverExecutionGuarantee,
) -> ResolverExecutionGuaranteeDisposition {
    policy
        .guarantees()
        .iter()
        .find(|row| row.guarantee() == guarantee)
        .expect("complete guarantee row")
        .disposition()
}

fn canonical_executable(path: &str) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|error| panic!("canonicalize {path}: {error}"))
}

fn temporary_directory(label: &str) -> PathBuf {
    let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "omega-resolver-linux-{label}-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&root).expect("create Linux resolver test root");
    fs::canonicalize(root).expect("canonicalize Linux resolver test root")
}
