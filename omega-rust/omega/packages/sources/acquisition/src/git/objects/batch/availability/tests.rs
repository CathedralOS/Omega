use super::*;
use crate::git::executable::budget::GitCapturedOutputBudget;
use crate::git::executable::executor::test_system_git_executor;
use crate::git::request::GitExecutionTransport;
use crate::limits::LocalSourceLimits;
use crate::snapshot::permissions::make_tree_owner_writable;
use crate::test_support::*;
use std::path::PathBuf;
use std::time::Duration;

#[test]
fn exact_response_accepts_only_full_oid_missing_or_closed_kind_and_size() {
    for length in [40, 64] {
        let oid = "a".repeat(length);
        assert_eq!(
            protocol::response(
                &oid,
                true,
                Some(0),
                format!("{oid} missing\n").as_bytes(),
                b""
            )
            .unwrap(),
            ExactGitObjectAvailability::Missing
        );
        for (label, kind) in [
            ("commit", ExactGitObjectKind::Commit),
            ("tree", ExactGitObjectKind::Tree),
            ("blob", ExactGitObjectKind::Blob),
            ("tag", ExactGitObjectKind::Tag),
        ] {
            for size in [0, 123, u64::MAX] {
                assert_eq!(
                    protocol::response(
                        &oid,
                        true,
                        Some(0),
                        format!("{oid} {label} {size}\n").as_bytes(),
                        b""
                    )
                    .unwrap(),
                    ExactGitObjectAvailability::Present { kind, size }
                );
            }
        }
    }
}

#[test]
fn malformed_wrong_object_and_failed_command_never_mean_missing() {
    let oid = "a".repeat(40);
    let missing = format!("{oid} missing\n");
    let present = format!("{oid} commit 42\n");
    for output in [&missing, &present] {
        for end in 0..output.len() {
            assert!(
                protocol::response(&oid, true, Some(0), &output.as_bytes()[..end], b"").is_err()
            );
        }
        assert!(protocol::response(&oid, true, Some(0), output.as_bytes(), b"warning\n").is_err());
        for status in [Some(1), Some(128), None] {
            assert!(matches!(
                protocol::response(&oid, false, status, output.as_bytes(), b"object missing"),
                Err(SourceResolveError::Git { .. })
            ));
        }
    }
    for output in [
        format!("{} missing\n", "b".repeat(40)),
        format!("{} commit 1\n", "b".repeat(40)),
        format!("{oid} missing\n{oid} missing\n"),
        format!("{oid} missing \n"),
        format!("{oid} missing\r\n"),
        format!("{oid} missing 0\n"),
        format!("{oid} dangling\n"),
        format!("{oid} unknown 1\n"),
        format!("{oid} tree 01\n"),
        format!("{oid} tree +1\n"),
        format!("{oid} tree -1\n"),
        format!("{oid} tree 18446744073709551616\n"),
        format!("{oid} blob 0\n\n"),
        format!("{oid} blob 0 0\n"),
    ] {
        assert!(
            protocol::response(&oid, true, Some(0), output.as_bytes(), b"").is_err(),
            "{output:?}"
        );
    }
    for requested in [
        "HEAD",
        "abc",
        "HEAD^{commit}",
        "--help",
        "\n",
        &"A".repeat(40),
    ] {
        assert!(
            protocol::response(
                requested,
                true,
                Some(0),
                format!("{requested} missing\n").as_bytes(),
                b""
            )
            .is_err()
        );
    }
}

struct Fixture {
    source: PathBuf,
    cache: PathBuf,
    repository: VerifiedGitRepository,
    commit: String,
    tree: String,
    blob: String,
}
impl Fixture {
    fn new(format: Option<&str>) -> Self {
        let (source, commit) = create_git_source_with_format("exact-object-probe", format);
        let blob = run_test_git_with_input(&source, ["rev-parse", "HEAD:main.omg"], b"");
        let cache = temp_root("exact-object-probe-cache");
        let request = local_git_request(&source, "HEAD");
        let resolved = resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect("prime verified object cache");
        let repository = open_verified_git_repository(&cache, &request);
        let offline = source.with_extension("offline");
        std::fs::rename(&source, &offline).expect("make all source transport unavailable");
        Self {
            source: offline,
            cache,
            repository,
            commit,
            tree: resolved.tree().to_owned(),
            blob,
        }
    }
    fn executor(&self) -> GitExecutor {
        test_system_git_executor(GitExecutionTransport::File).expect("frozen fixture Git")
    }
    fn assert_no_temporary_request(&self) {
        for entry in std::fs::read_dir(self.repository.entry_root.parent().unwrap()).unwrap() {
            let name = entry.unwrap().file_name();
            assert!(!name.to_string_lossy().starts_with(".omega-cat-file-batch."));
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.source);
        make_tree_owner_writable(&self.cache);
        let _ = std::fs::remove_dir_all(&self.cache);
    }
}

#[test]
fn verified_probe_uses_one_bounded_launch_and_preserves_offline_objects() {
    for format in [None, Some("sha256")] {
        let fixture = Fixture::new(format);
        let executor = fixture.executor();
        for (oid, expected) in [
            (&fixture.commit, ExactGitObjectKind::Commit),
            (&fixture.tree, ExactGitObjectKind::Tree),
            (&fixture.blob, ExactGitObjectKind::Blob),
        ] {
            let before = executor.launches.get();
            let ExactGitObjectAvailability::Present { kind, size } =
                probe_exact_git_object(&executor, &fixture.repository, oid).unwrap()
            else {
                panic!("verified object remains available")
            };
            assert_eq!(kind, expected);
            assert!(size > 0);
            assert_eq!(executor.launches.get(), before + 1);
            fixture.assert_no_temporary_request();
        }
        let missing = "0".repeat(fixture.commit.len());
        let before = executor.launches.get();
        assert_eq!(
            probe_exact_git_object(&executor, &fixture.repository, &missing).unwrap(),
            ExactGitObjectAvailability::Missing
        );
        assert_eq!(executor.launches.get(), before + 1);
        assert!(executor.captured_output_budget.observed() > 0);
        fixture
            .repository
            .verify_current(LocalSourceLimits::default())
            .unwrap();
        fixture.assert_no_temporary_request();
    }
}

#[test]
fn command_time_and_output_limits_are_errors_not_availability() {
    let fixture = Fixture::new(None);
    let mut executor = fixture.executor();
    executor.maximum_launches = 0;
    assert_eq!(
        probe_exact_git_object(&executor, &fixture.repository, &fixture.commit),
        Err(SourceResolveError::GitResolutionCommandLimit { limit: 0 })
    );
    fixture.assert_no_temporary_request();

    let mut executor = fixture.executor();
    executor.timeout = Duration::ZERO;
    assert!(matches!(
        probe_exact_git_object(&executor, &fixture.repository, &fixture.commit),
        Err(SourceResolveError::GitResolutionTimedOut { .. })
    ));
    assert_eq!(executor.launches.get(), 0);
    fixture.assert_no_temporary_request();

    let mut executor = fixture.executor();
    executor.captured_output_budget = GitCapturedOutputBudget::new(0);
    let error =
        probe_exact_git_object(&executor, &fixture.repository, &fixture.commit).unwrap_err();
    // The process owner gives a cleanup failure precedence over an output
    // overflow when the host refuses process-container termination. Both must
    // remain errors; neither can authorize an exact-revision fetch.
    match error {
        SourceResolveError::GitResolutionCapturedOutputLimit {
            ceiling: 0,
            attempted,
        } => assert!(attempted > 0),
        SourceResolveError::GitCleanupFailed { operation, .. } => assert_eq!(operation, OPERATION),
        other => panic!("expected bounded output rejection or its cleanup failure: {other:?}"),
    }
    fixture.assert_no_temporary_request();

    let executor = fixture.executor();
    assert!(probe_exact_git_object(&executor, &fixture.repository, "HEAD").is_err());
    assert_eq!(executor.launches.get(), 0);
    fixture.assert_no_temporary_request();
}

#[test]
#[cfg_attr(
    windows,
    ignore = "Windows prevents replacing the open retained cache directory"
)]
fn detached_repository_custody_rejects_before_probe_launch() {
    let fixture = Fixture::new(None);
    let executor = fixture.executor();
    let displaced = fixture.repository.entry_root.with_extension("displaced");
    std::fs::rename(&fixture.repository.entry_root, &displaced)
        .expect("detach retained cache entry");
    std::fs::create_dir(&fixture.repository.entry_root).expect("replace cache entry name");
    assert!(matches!(
        probe_exact_git_object(&executor, &fixture.repository, &fixture.commit),
        Err(SourceResolveError::GitCacheInvalid { .. })
    ));
    assert_eq!(executor.launches.get(), 0);
    fixture.assert_no_temporary_request();
}
