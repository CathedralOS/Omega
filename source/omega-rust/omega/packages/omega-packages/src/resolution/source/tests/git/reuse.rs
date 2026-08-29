use super::*;

#[test]
fn exact_git_revision_reuses_authenticated_objects_without_transport() {
    let (repo, commit) = create_git_source("git-exact-offline-reuse");
    let cache = temp_root("git-exact-offline-reuse-cache");
    let request = local_git_request(&repo, &commit);
    let first = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect("resolve exact revision");
    let offline_repo = repo.with_extension("offline");
    std::fs::rename(&repo, &offline_repo).expect("make source transport unavailable");

    let second = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect("reuse exact resolver custody without transport");

    assert_eq!(second.commit, first.commit);
    assert_eq!(second.tree, first.tree);
    assert_eq!(second.snapshot_root, first.snapshot_root);
    assert_eq!(second.local, first.local);

    let _ = std::fs::remove_dir_all(&offline_repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn exact_git_revision_offline_reuse_still_enforces_source_limits() {
    let (repo, commit) = create_git_source("git-exact-offline-limits");
    let cache = temp_root("git-exact-offline-limits-cache");
    let request = local_git_request(&repo, &commit);
    resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect("resolve exact revision");
    let offline_repo = repo.with_extension("offline");
    std::fs::rename(&repo, &offline_repo).expect("make source transport unavailable");

    let error = resolve_git_source(
        &request,
        &cache,
        LocalSourceLimits {
            max_bytes: 0,
            ..LocalSourceLimits::default()
        },
    )
    .expect_err("cached exact source must remain subject to current limits");

    assert_eq!(error, SourceResolveError::TooManyBytes { limit: 0 });

    let _ = std::fs::remove_dir_all(&offline_repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn symbolic_git_revision_still_refetches_and_observes_movement() {
    let (repo, first_commit) = create_git_source("git-symbolic-refresh");
    let cache = temp_root("git-symbolic-refresh-cache");
    let request = local_git_request(&repo, "HEAD");
    let first = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect("resolve initial symbolic revision");
    assert_eq!(first.commit, first_commit);

    std::fs::write(repo.join("main.omg"), "machine Main::changed() {}\n").expect("change source");
    run_test_git(&repo, ["add", "main.omg"]);
    run_test_git(&repo, ["commit", "--quiet", "-m", "move symbolic revision"]);
    let second_commit = run_test_git_with_input(&repo, ["rev-parse", "HEAD"], b"");

    let second = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect("refresh symbolic revision");

    assert_eq!(second.commit, second_commit);
    assert_ne!(second.commit, first.commit);
    assert_eq!(
        std::fs::read(second.snapshot_root.join("main.omg")).expect("read refreshed source"),
        b"machine Main::changed() {}\n"
    );

    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}
