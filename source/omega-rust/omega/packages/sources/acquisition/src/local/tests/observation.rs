use super::*;

#[test]
fn local_resolution_observation_binds_request_spelling_and_limits() {
    let root = temp_root("local-observation-request");
    let cache = temp_root("local-observation-request-cache");
    let spelling_anchor = root
        .parent()
        .expect("temporary source has a parent")
        .join(format!(
            "{}-spelling-anchor",
            root.file_name()
                .expect("temporary source has a name")
                .to_string_lossy()
        ));
    std::fs::create_dir_all(&root).expect("create source");
    std::fs::create_dir_all(&spelling_anchor).expect("create request spelling anchor");
    std::fs::write(root.join("main.omg"), "machine Main::main() {}\n").expect("write source");

    let ordinary_limits = LocalSourceLimits::default();
    let alternate_request = spelling_anchor.join("..").join(
        root.file_name()
            .expect("temporary source has a final component"),
    );
    let ordinary = resolve_local_source_snapshot(&root, &cache, ordinary_limits)
        .expect("resolve ordinary request");
    let alternate = resolve_local_source_snapshot(&alternate_request, &cache, ordinary_limits)
        .expect("resolve alternate request spelling");
    assert_eq!(ordinary.snapshot_root(), alternate.snapshot_root());
    assert_eq!(
        ordinary.resolution_observation().custody_identity(),
        alternate.resolution_observation().custody_identity()
    );
    assert_ne!(
        ordinary.resolution_observation().identity(),
        alternate.resolution_observation().identity(),
        "the exact caller request must remain in final provenance"
    );

    let tighter_limits = LocalSourceLimits {
        max_entries: ordinary_limits.max_entries - 1,
        max_bytes: ordinary_limits.max_bytes,
        max_depth: ordinary_limits.max_depth,
    };
    let tighter = resolve_local_source_snapshot(&root, &cache, tighter_limits)
        .expect("resolve under tighter accepted limits");
    assert_eq!(ordinary.snapshot_root(), tighter.snapshot_root());
    assert_ne!(
        ordinary.resolution_observation().identity(),
        tighter.resolution_observation().identity(),
        "accepted source ceilings must remain in final provenance"
    );
    assert_eq!(ordinary.resolution_observation().schema_version(), 2);
    assert_eq!(ordinary.resolution_observation().identity().len(), 64);

    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&cache);
    let _ = std::fs::remove_dir_all(&spelling_anchor);
}

#[test]
fn local_snapshot_issuance_rejects_a_mismatched_requested_root() {
    let captured_root = temp_root("local-observation-captured-root");
    let substituted_request = temp_root("local-observation-substituted-request");
    let cache = temp_root("local-observation-substituted-cache");
    for root in [&captured_root, &substituted_request] {
        std::fs::create_dir_all(root).expect("create source root");
        std::fs::write(root.join("main.omg"), "machine Main::main() {}\n")
            .expect("write identical source");
    }
    let captured = capture_local_source(
        &captured_root,
        LocalSourceLimits::default(),
        SourceTreePolicy::LocalPackage,
    )
    .expect("capture source");

    let error = publish_local_snapshot(
        substituted_request.clone(),
        captured,
        &cache,
        LocalSourceLimits::default(),
    )
    .expect_err("final issuance must reconcile the request to the captured source");
    assert!(matches!(
        error,
        SourceResolveError::LocalSourceChanged { path } if path == substituted_request
    ));

    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&captured_root);
    let _ = std::fs::remove_dir_all(&substituted_request);
    let _ = std::fs::remove_dir_all(&cache);
}
