use super::*;

#[test]
fn git_source_fetches_only_the_selected_revision_depth() {
    let (repo, _) = create_git_source("git-shallow");
    std::fs::write(repo.join("main.omg"), "machine Main::changed() {}\n").expect("change source");
    run_test_git(&repo, ["add", "main.omg"]);
    run_test_git(&repo, ["commit", "--quiet", "-m", "second"]);
    let cache = temp_root("git-shallow-cache");
    let request = local_git_request(&repo, "HEAD");

    resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect("resolve a shallow exact revision");

    let repository = git_cache_entry_root(&cache, &request).join(GIT_CACHE_REPOSITORY);
    let output = Command::new("git")
        .args(["-c", "core.longpaths=true"])
        .arg("-C")
        .arg(&repository)
        .args(["rev-list", "--count", "FETCH_HEAD"])
        .output()
        .expect("count fetched history");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "1");
    assert!(repository.join("shallow").is_file());

    let _ = std::fs::remove_dir_all(&repo);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_fetch_request_is_depth_one_and_omits_individually_inadmissible_blobs() {
    let arguments = bounded_git_fetch_arguments(
        "https://example.invalid/package.git",
        "0123456789012345678901234567890123456789",
        LocalSourceLimits {
            max_bytes: 4096,
            ..LocalSourceLimits::default()
        },
    );
    let arguments = arguments
        .iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(
        arguments,
        [
            "fetch",
            "--quiet",
            "--depth=1",
            "--no-tags",
            "--no-recurse-submodules",
            "--filter=blob:limit=4097",
            "--",
            "https://example.invalid/package.git",
            "0123456789012345678901234567890123456789",
        ]
    );
}

#[test]
fn git_fetch_omits_a_blob_above_the_source_byte_ceiling_and_rejects() {
    let (repo, _) = create_git_source("git-filtered-oversized-blob");
    std::fs::write(repo.join("oversized.bin"), vec![0x5a; 4096]).expect("write oversized blob");
    run_test_git(&repo, ["add", "oversized.bin"]);
    run_test_git(&repo, ["commit", "--quiet", "-m", "add oversized blob"]);
    run_test_git(&repo, ["config", "uploadpack.allowFilter", "true"]);
    let commit = run_test_git_with_input(&repo, ["rev-parse", "HEAD"], b"");
    let oversized_blob = run_test_git_with_input(&repo, ["rev-parse", "HEAD:oversized.bin"], b"");
    let cache = temp_root("git-filtered-oversized-blob-cache");
    let mut request = local_git_request(&repo, &commit);
    request.fetch_locator = format!("file://{}", repo.display());

    let limits = LocalSourceLimits {
        max_bytes: 1024,
        ..LocalSourceLimits::default()
    }
    .compiler_bounded();
    std::fs::create_dir_all(&cache).expect("create resolver cache");
    let canonical_cache = cache.canonicalize().expect("canonical resolver cache");
    verify_git_cache_root_custody(&canonical_cache).expect("verify resolver cache custody");
    let execution_transport = request.execution_transport();
    let executor = test_system_git_executor(execution_transport).expect("select test Git executor");
    let cache_identity = git_cache_identity(
        request.locator_identity(),
        request.requested_revision(),
        execution_transport,
    );
    let entry_root = canonical_cache.join(format!("git-{cache_identity}"));
    let cache_directory =
        open_absolute_directory_nofollow(&canonical_cache).expect("retain resolver cache");
    let entry_name = entry_root.file_name().expect("cache entry has a name");
    create_git_cache_entry(
        &executor,
        &canonical_cache,
        &cache_directory,
        &entry_root,
        entry_name,
        &cache_identity,
        request.locator_identity(),
        request.fetch_locator(),
        request.requested_revision(),
        execution_transport,
        limits,
    )
    .expect("create quarantined Git cache entry");
    let error = resolve_verified_git_cache_entry(
        &executor,
        &cache_directory,
        entry_name,
        &entry_root,
        request.requested_locator(),
        request.lineage(),
        request.locator_identity(),
        request.fetch_locator(),
        request.requested_revision(),
        execution_transport,
        limits,
        true,
    )
    .expect_err("a required blob above the source ceiling must not be acquired");

    assert!(matches!(error, SourceResolveError::GitTreeInvalid { .. }));
    let repository = entry_root.join(GIT_CACHE_REPOSITORY);
    let output = Command::new("git")
        .env("GIT_NO_LAZY_FETCH", "1")
        .arg("-C")
        .arg(&repository)
        .args(["cat-file", "-e", &oversized_blob])
        .output()
        .expect("inspect quarantined object store");
    assert!(
        !output.status.success(),
        "the inadmissible blob must remain absent from resolver custody"
    );
    assert!(entry_root.join(GIT_CACHE_METADATA).exists());
    assert!(!entry_root.join(GIT_CACHE_SNAPSHOTS).exists());

    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}
