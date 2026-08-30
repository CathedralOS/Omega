use super::*;

#[test]
fn review_compilation_rejects_snapshot_tampering_before_compiler_consumption() {
    let fixtures = workspace_root().join("tests/fixtures/packages");
    let workspace_lineage = SourceLineage::git("https://github.com/CathedralOS/Omega.git").unwrap();
    let cache = temp_root("tampered-custody");
    let closure = resolve_workspace_package_closure(
        &workspace_lineage,
        SourceRelativePath::parse("arithmetic-kernels").unwrap(),
        &fixtures,
        &cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("fixture source closure should resolve");
    let root = closure.graph().root().clone();
    let main = closure
        .source_root(&root)
        .expect("root custody")
        .join("main.omg");
    let mut permissions = std::fs::metadata(&main).unwrap().permissions();
    permissions.set_readonly(false);
    std::fs::set_permissions(&main, permissions).unwrap();
    std::fs::write(&main, b"pub machine altered() -> u32 { 0 }\n").unwrap();
    let mut permissions = std::fs::metadata(&main).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&main, permissions).unwrap();

    let error =
        compile_resolved_package_reviews(&closure, "windows_x64", &cache.join("compiler-build"))
            .expect_err("tampered resolver custody must not reach compilation");

    assert!(matches!(
        error,
        CompileResolvedPackageReviewsError::SourceCustody {
            source_package,
            phase: PackageSourceVerificationPhase::BeforeCompilation,
            error: SourceResolveError::SourceSnapshotContentMismatch { .. },
            ..
        } if source_package == root
    ));
    assert!(
        std::fs::read_dir(cache.join("compiler-build"))
            .expect("review build workspace remains readable")
            .next()
            .is_none(),
        "failed review must dispose its private build session"
    );
    let _ = std::fs::remove_dir_all(cache);
}
