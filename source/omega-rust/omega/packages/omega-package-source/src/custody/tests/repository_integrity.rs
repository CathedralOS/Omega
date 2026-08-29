use super::{
    GIT_CACHE_METADATA, GIT_CACHE_REPOSITORY, GIT_CONFIG_SHA1, GitExecutionTransport,
    LocalSourceLimits, ProvisionalCacheDirectory, SourceResolveError, create_git_source,
    create_private_cache_directory, git_cache_entry_root, git_cache_metadata, local_git_request,
    make_tree_owner_writable, open_absolute_directory_nofollow, open_verified_git_repository,
    resolve_git_source, temp_root,
};
#[cfg(unix)]
use super::{
    first_regular_descendant, invalidate_git_cache_entry_from_retained_parent,
    verify_git_cache_root_custody,
};
use std::ffi::OsStr;

#[test]
fn provisional_git_cache_directory_is_cleaned_if_retention_fails() {
    let cache = temp_root("git-provisional-stage-cleanup");
    std::fs::create_dir_all(&cache).expect("create provisional cache parent");
    let cache = cache.canonicalize().expect("canonicalize cache parent");
    let parent = open_absolute_directory_nofollow(&cache).expect("retain cache parent");
    create_private_cache_directory(&parent, "provisional")
        .expect("create provisional cache directory");
    {
        let _provisional = ProvisionalCacheDirectory::new(&parent, OsStr::new("provisional"));
        // Returning from a failed retention path drops this guard while it
        // still owns the just-created parent-relative name.
    }
    assert!(!cache.join("provisional").exists());
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_cache_rejects_resolver_metadata_substitution() {
    let (repo, _) = create_git_source("git-metadata-source");
    let (substitute, _) = create_git_source("git-metadata-substitute");
    let cache = temp_root("git-metadata-cache");
    let substitute_url = substitute.display().to_string();
    let request = local_git_request(&repo, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
    let entry = git_cache_entry_root(&cache, &request);
    std::fs::write(
        entry.join(GIT_CACHE_METADATA),
        git_cache_metadata(&substitute_url, "HEAD", GitExecutionTransport::File),
    )
    .expect("substitute metadata");

    let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect_err("substituted metadata must reject");

    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
    assert!(!entry.join(GIT_CACHE_METADATA).exists());
    let _ = std::fs::remove_dir_all(&repo);
    let _ = std::fs::remove_dir_all(&substitute);
    let _ = std::fs::remove_dir_all(&cache);
}

#[cfg(unix)]
#[test]
fn git_cache_invalidation_does_not_follow_a_substituted_entry_symlink() {
    let cache = temp_root("git-invalidation-symlink");
    let target = cache.join("target");
    let entry = cache.join("git-substituted");
    std::fs::create_dir_all(&target).expect("create invalidation target");
    let target_metadata = target.join(GIT_CACHE_METADATA);
    std::fs::write(&target_metadata, b"must remain").expect("write target metadata");
    std::os::unix::fs::symlink(&target, &entry).expect("substitute Git cache entry");

    let error = invalidate_git_cache_entry_from_retained_parent(&entry)
        .expect_err("invalidation must reject a substituted entry symlink");

    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
    assert_eq!(
        std::fs::read(&target_metadata).expect("read retained target metadata"),
        b"must remain"
    );
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_cache_rejects_transport_profile_substitution() {
    let (repo, _) = create_git_source("git-transport-metadata-source");
    let cache = temp_root("git-transport-metadata-cache");
    let request = local_git_request(&repo, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
    let entry = git_cache_entry_root(&cache, &request);
    std::fs::write(
        entry.join(GIT_CACHE_METADATA),
        git_cache_metadata(
            request.locator_identity(),
            request.requested_revision(),
            GitExecutionTransport::Https,
        ),
    )
    .expect("substitute transport profile metadata");

    let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect_err("substituted transport profile must reject");

    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
    assert!(!entry.join(GIT_CACHE_METADATA).exists());
    let _ = std::fs::remove_dir_all(&repo);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_cache_rejects_repository_config_substitution_without_asking_git() {
    let (repo, _) = create_git_source("git-origin-source");
    let cache = temp_root("git-origin-cache");
    let request = local_git_request(&repo, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
    let repository = git_cache_entry_root(&cache, &request).join(GIT_CACHE_REPOSITORY);
    let config = repository.join("config");
    assert_eq!(std::fs::read(&config).unwrap(), GIT_CONFIG_SHA1);
    let mut substituted = GIT_CONFIG_SHA1.to_vec();
    substituted.extend_from_slice(b"[remote \"origin\"]\n\turl = /substitute\n");
    std::fs::write(&config, substituted).expect("substitute repository config");
    let entry = git_cache_entry_root(&cache, &request);

    let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect_err("any noncanonical repository configuration must reject");

    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
    assert!(!entry.join(GIT_CACHE_METADATA).exists());
    let _ = std::fs::remove_dir_all(&repo);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn verified_git_repository_rejects_replaced_repository_path() {
    let (repo, _) = create_git_source("git-retained-repository-source");
    let cache = temp_root("git-retained-repository-cache");
    let request = local_git_request(&repo, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
    let verified = open_verified_git_repository(&cache, &request);
    let repository = verified.path().to_path_buf();
    let displaced = repository.with_file_name("repository.displaced");
    std::fs::rename(&repository, &displaced).expect("displace retained repository");
    std::fs::create_dir_all(repository.join("objects")).expect("create replacement repository");

    let error = verified
        .verify_identity()
        .expect_err("repository replacement must reject");
    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));

    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn verified_git_repository_rejects_replaced_objects_path() {
    let (repo, _) = create_git_source("git-retained-objects-source");
    let cache = temp_root("git-retained-objects-cache");
    let request = local_git_request(&repo, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
    let verified = open_verified_git_repository(&cache, &request);
    let objects = verified.path().join("objects");
    let displaced = verified.path().join("objects.displaced");
    std::fs::rename(&objects, &displaced).expect("displace retained object store");
    std::fs::create_dir(&objects).expect("create replacement object store");

    let error = verified
        .verify_identity()
        .expect_err("object-store replacement must reject");
    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));

    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_cache_forbidden_record_probe_rejects_non_not_found_errors() {
    let (repo, _) = create_git_source("git-forbidden-probe-source");
    let cache = temp_root("git-forbidden-probe-cache");
    let request = local_git_request(&repo, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
    let entry = git_cache_entry_root(&cache, &request);
    let info = entry.join(GIT_CACHE_REPOSITORY).join("objects/info");
    std::fs::remove_dir(&info).expect("remove empty Git info directory");
    std::fs::write(&info, b"not a directory").expect("replace info with a regular file");

    let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect_err("NotADirectory must not prove a forbidden record absent");
    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));

    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[cfg(unix)]
#[test]
fn git_cache_rejects_symlinks_in_owned_repository_namespaces() {
    for relative in ["config", "FETCH_HEAD", "HEAD"] {
        let (repo, _) = create_git_source(&format!("git-symlink-{relative}-source"));
        let cache = temp_root(&format!("git-symlink-{relative}-cache"));
        let request = local_git_request(&repo, "HEAD");
        resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
        let entry = git_cache_entry_root(&cache, &request);
        let repository = entry.join(GIT_CACHE_REPOSITORY);
        let path = repository.join(relative);
        let displaced = repository.join(format!("{relative}.displaced"));
        std::fs::rename(&path, &displaced).expect("displace repository file");
        std::os::unix::fs::symlink(&displaced, &path).expect("install repository symlink");

        let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect_err("repository symlink must reject");
        assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));

        let _ = std::fs::remove_dir_all(&repo);
        make_tree_owner_writable(&cache);
        let _ = std::fs::remove_dir_all(&cache);
    }

    let (repo, _) = create_git_source("git-symlink-object-source");
    let cache = temp_root("git-symlink-object-cache");
    let request = local_git_request(&repo, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
    let repository = git_cache_entry_root(&cache, &request).join(GIT_CACHE_REPOSITORY);
    let object = first_regular_descendant(&repository.join("objects"));
    let displaced = object.with_extension("displaced");
    std::fs::rename(&object, &displaced).expect("displace object payload");
    std::os::unix::fs::symlink(&displaced, &object).expect("install object symlink");

    let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect_err("object-store symlink must reject");
    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));

    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[cfg(unix)]
#[test]
fn git_cache_rejects_multiply_linked_regular_files() {
    let (repo, _) = create_git_source("git-hardlink-source");
    let cache = temp_root("git-hardlink-cache");
    let request = local_git_request(&repo, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
    let entry = git_cache_entry_root(&cache, &request);
    let config = entry.join(GIT_CACHE_REPOSITORY).join("config");
    std::fs::hard_link(&config, cache.join("config-alias"))
        .expect("add external hard link to repository file");

    let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect_err("multiply-linked repository file must reject");
    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));

    let _ = std::fs::remove_dir_all(&repo);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[cfg(unix)]
#[test]
fn git_cache_rejects_group_or_other_writable_custody() {
    use std::os::unix::fs::PermissionsExt;

    let (repo, _) = create_git_source("git-custody-source");
    let cache = temp_root("git-custody-cache");
    let request = local_git_request(&repo, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
    std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(0o777))
        .expect("make cache externally writable");

    let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect_err("externally writable cache custody must reject");
    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));

    std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(0o700)).unwrap();
    let _ = std::fs::remove_dir_all(&repo);
    let _ = std::fs::remove_dir_all(&cache);
}

#[cfg(unix)]
#[test]
fn cache_custody_rejects_replaceable_nonsticky_ancestry() {
    use std::os::unix::fs::PermissionsExt;

    let parent = temp_root("replaceable-cache-parent");
    let cache = parent.join("cache");
    std::fs::create_dir_all(&cache).expect("create nested cache");
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o777))
        .expect("make parent replaceable");
    std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(0o700))
        .expect("keep cache itself private");

    assert!(matches!(
        verify_git_cache_root_custody(&cache),
        Err(SourceResolveError::GitCacheInvalid { .. })
    ));

    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
    let _ = std::fs::remove_dir_all(&parent);
}
