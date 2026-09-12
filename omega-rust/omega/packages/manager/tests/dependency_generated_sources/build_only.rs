use super::*;

struct BuildOnlyFixture {
    root: PathBuf,
}

impl BuildOnlyFixture {
    fn new() -> Self {
        let root = temporary_root();
        for name in ["consumer", "dependency"] {
            std::fs::create_dir_all(root.join(name)).unwrap();
        }
        std::fs::write(
            root.join("consumer/build.omg"),
            "machine build(builder: &mut Build) {
                builder.package(\"consumer\");
                builder.depend(Source::Path { location: \"../dependency\" });
            }",
        )
        .unwrap();
        std::fs::write(
            root.join("consumer/main.omg"),
            "pub data Value { value: u64; }",
        )
        .unwrap();
        std::fs::write(
            root.join("dependency/build.omg"),
            "machine build(builder: &mut Build) { builder.package(\"dependency\"); }",
        )
        .unwrap();
        Self { root }
    }

    fn closure(&self) -> Result<ResolvedPackageSourceClosure, ResolveWorkspacePackageClosureError> {
        resolve_workspace_package_closure(
            &SourceLineage::git("https://github.com/CathedralOS/Omega.git").unwrap(),
            SourceRelativePath::parse("consumer").unwrap(),
            &self.root,
            self.root.join("cache"),
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
    }
}

impl Drop for BuildOnlyFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn build_only_dependency_receives_its_own_source_bound_review() {
    let fixture = BuildOnlyFixture::new();
    let closure = fixture.closure().unwrap();
    let reviews = compile_resolved_package_reviews(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        &fixture.root.join("review"),
    )
    .expect("review authored build-only dependency");
    assert_eq!(reviews.reviews().len(), 2);
    let dependency = reviews
        .reviews()
        .iter()
        .find(|review| review.key().name().as_str() == "dependency")
        .unwrap();
    assert!(dependency.generated_source_bundle().sources().is_empty());
    assert!(
        dependency
            .policy()
            .callables()
            .callables()
            .iter()
            .all(|callable| callable.role()
                != package_evidence::record::PackagePolicyCallableRole::Public)
    );
    assert_eq!(dependency.policy().package(), dependency.key().identity());
    assert!(!fixture.root.join("dependency/main.omg").exists());
    assert!(!fixture.root.join("consumer/omega.lock").exists());
}

#[test]
fn build_only_review_does_not_supply_missing_imports_or_hide_invalid_main() {
    for (file, source) in [
        (
            "consumer/main.omg",
            "use dependency::missing; pub data Value {}",
        ),
        ("dependency/main.omg", "this is not Omega source"),
        (
            "dependency/build.omg",
            "machine build(builder: &mut Build) { builder.package(\"dependency\"); missing(); }",
        ),
    ] {
        let fixture = BuildOnlyFixture::new();
        std::fs::write(fixture.root.join(file), source).unwrap();
        let closure = fixture.closure().unwrap();
        let result = compile_resolved_package_reviews(
            &closure.for_exact_target(target::TargetProfile::WindowsX64),
            &fixture.root.join("review"),
        );
        assert!(
            matches!(
                result,
                Err(CompileResolvedPackageReviewsError::Compilation { .. })
            ),
            "{file}"
        );
    }
}

#[test]
fn missing_build_only_declaration_rejects_before_review() {
    let fixture = BuildOnlyFixture::new();
    std::fs::remove_file(fixture.root.join("dependency/build.omg")).unwrap();
    assert!(fixture.closure().is_err());
}
