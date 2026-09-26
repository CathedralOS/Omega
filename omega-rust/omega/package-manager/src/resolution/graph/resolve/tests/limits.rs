use super::super::{
    LocalSourceLimits, PackageSourceClosureLimits, PackageSourceClosureResolutionError,
    ResolveWorkspacePackageClosureError, SourceRelativePath,
    resolve_workspace_package_closure_from_hardened_base,
};
use super::{PackageSourceClosureLimitKind, fixture_lineage, fixture_root, temp_root};
#[test]
fn propagates_closure_resource_ceilings() {
    let cache = temp_root("limit-cache");
    let error = resolve_workspace_package_closure_from_hardened_base(
        &fixture_lineage(),
        SourceRelativePath::parse("graph-workbench").expect("root member"),
        fixture_root(),
        &cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits {
            max_packages: 1,
            max_dependency_requests: 8,
            max_depth: 8,
        },
    )
    .expect_err("package ceiling must reject");

    assert!(matches!(
        error,
        ResolveWorkspacePackageClosureError::Closure(
            PackageSourceClosureResolutionError::LimitExceeded {
                kind: PackageSourceClosureLimitKind::Packages,
                ..
            }
        )
    ));

    let _ = std::fs::remove_dir_all(cache);
}
