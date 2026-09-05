use super::{make_tree_owner_writable, temp_root, write_package};
use crate::resolution::source::resolve_external_local_package_source;
use package_source::{
    ExternalSourceContext, ImmutableSourceResolution, LocalSourceLimits, SourceLineage,
};

#[test]
fn external_local_resolution_uses_declared_name_and_immutable_snapshot() {
    let root = temp_root("external");
    let cache = temp_root("external-cache");
    write_package(&root, "arithmetic-kernels");

    let resolved = resolve_external_local_package_source(
        &root,
        &cache,
        LocalSourceLimits::default(),
        ExternalSourceContext::derive(b"consumer-lock"),
    )
    .expect("resolve declared local package");

    assert_eq!(resolved.key().name().as_str(), "arithmetic-kernels");
    assert!(matches!(
        resolved.key().source_lineage(),
        SourceLineage::ExternalLocal(_)
    ));
    assert!(matches!(
        resolved.resolution(),
        ImmutableSourceResolution::ExternalLocal { .. }
    ));
    assert!(resolved.dependency_requests().is_empty());
    assert_ne!(
        resolved.snapshot_root(),
        root.canonicalize().expect("canonical live root")
    );
    assert_eq!(resolved.snapshot_root(), resolved.source().snapshot_root());
    let _ = std::fs::remove_dir_all(&root);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn external_local_context_changes_key_without_changing_source_resolution() {
    let root = temp_root("context");
    let cache = temp_root("context-cache");
    write_package(&root, "arithmetic-kernels");

    let first = resolve_external_local_package_source(
        &root,
        &cache,
        LocalSourceLimits::default(),
        ExternalSourceContext::derive(b"consumer-a"),
    )
    .expect("resolve first context");
    let second = resolve_external_local_package_source(
        &root,
        &cache,
        LocalSourceLimits::default(),
        ExternalSourceContext::derive(b"consumer-b"),
    )
    .expect("resolve second context");

    assert_ne!(first.key(), second.key());
    assert_eq!(first.resolution(), second.resolution());
    assert_eq!(first.snapshot_root(), second.snapshot_root());

    let _ = std::fs::remove_dir_all(&root);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}
