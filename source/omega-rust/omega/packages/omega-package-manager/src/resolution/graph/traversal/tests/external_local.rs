use super::*;

#[test]
fn resolves_external_local_closure_across_directory_boundaries_in_one_context() {
    let sources = temp_root("external-sources");
    let first_cache = temp_root("external-first-cache");
    let second_cache = temp_root("external-second-cache");
    write_package(&sources.join("root"), "root-package", Some("../middle"));
    let leaf = sources.join("leaf");
    let leaf_location = leaf.display().to_string();
    write_package(
        &sources.join("middle"),
        "middle-package",
        Some(&leaf_location),
    );
    write_package(&leaf, "leaf-package", None);
    let first_context = ExternalSourceContext::derive(b"first-consuming-lock");

    let first = resolve_external_local_package_closure(
        sources.join("root"),
        first_context.clone(),
        &first_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve context-bound external closure");

    assert_eq!(first.graph().packages().len(), 3);
    assert!(first.custodies().iter().all(|custody| {
        matches!(
            custody.key().source_lineage(),
            SourceLineage::ExternalLocal(lineage)
                if lineage.source_context() == &first_context
        )
    }));
    let first_root_binding = first.source_requests().root();
    let PackageRootSourceRequest::ExternalLocal {
        requested_root,
        source_context,
    } = first_root_binding.request()
    else {
        panic!("external adapter retains its root request")
    };
    assert_eq!(requested_root, &sources.join("root"));
    assert_eq!(source_context, &first_context);

    let second_context = ExternalSourceContext::derive(b"second-consuming-lock");
    let second = resolve_external_local_package_closure(
        sources.join("root"),
        second_context,
        &second_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve same sources in a different consuming context");
    for first_custody in first.custodies() {
        let second_custody = second
            .custodies()
            .iter()
            .find(|custody| custody.key().name() == first_custody.key().name())
            .expect("same declared package in second closure");
        assert_ne!(first_custody.key(), second_custody.key());
        assert_eq!(first_custody.resolution(), second_custody.resolution());
    }

    let _ = std::fs::remove_dir_all(sources);
    let _ = std::fs::remove_dir_all(first_cache);
    let _ = std::fs::remove_dir_all(second_cache);
}
