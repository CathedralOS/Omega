use super::*;
use crate::declarations::BuildDeclarationKind;
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits, GitDependencyPins,
    resolve_staged_external_local_project_closure_with_git_pins,
};
use crate::resolution::source::ResolvePackageSourceError;
use package_source::git::resolution::GitExactRevisionAcquisition;
use package_source::local::staging::{StagedLocalSnapshot, stage_local_source_replacement_in_lane};
use package_source::{ExternalLocalLineage, SourceResolveError};
use sha2::{Digest, Sha256};

struct Fixture(PathBuf);

impl Fixture {
    fn new(name: &str) -> Self {
        Self(temp_root(name))
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }

    fn stage(&self, storage: &SourceResolverStorage, proposed: &str) -> StagedLocalSnapshot {
        let requested_root = self.path("root/../root");
        let original = std::fs::read(requested_root.join("build.omg")).unwrap();
        stage_local_source_replacement_in_lane(
            &requested_root,
            &SourceRelativePath::parse("build.omg").unwrap(),
            &Sha256::digest(&original).into(),
            proposed.as_bytes(),
            storage.external_local_sources(),
            LocalSourceLimits::default(),
        )
        .expect("stage build dependency edit")
    }
}

impl Drop for Fixture {
    #[cfg_attr(windows, allow(clippy::permissions_set_readonly_false))]
    fn drop(&mut self) {
        let mut pending = vec![self.0.clone()];
        while let Some(path) = pending.pop() {
            let Ok(metadata) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.file_type().is_symlink() {
                continue;
            }
            #[cfg(unix)]
            if metadata.is_dir() {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700));
            }
            #[cfg(windows)]
            {
                let mut permissions = metadata.permissions();
                permissions.set_readonly(false);
                let _ = std::fs::set_permissions(&path, permissions);
            }
            if metadata.is_dir()
                && let Ok(entries) = std::fs::read_dir(path)
            {
                pending.extend(entries.flatten().map(|entry| entry.path()));
            }
        }
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn staged_project_adds_relative_and_nested_path_dependencies_from_live_directories() {
    let fixture = Fixture::new("staged-closure-paths");
    write_application(&fixture.path("root"), "staged-app", None);
    write_package(&fixture.path("middle"), "middle", Some("./nested/leaf"));
    write_package(&fixture.path("middle/nested/leaf"), "leaf", None);
    write_package(&fixture.path("root/nested"), "nested", None);
    let original_build = std::fs::read(fixture.path("root/build.omg")).unwrap();
    let original_main = std::fs::read(fixture.path("root/main.omg")).unwrap();
    let storage = SourceResolverStorage::for_hardened_base(fixture.path("cache")).unwrap();
    let context = ExternalSourceContext::derive(b"staged-closure-context");
    let proposed = "machine build(builder: &mut Build) {\n    builder.application(\"staged-app\");\n    builder.depend(Source::Path { location: \"../middle\" });\n    builder.depend(Source::Path { location: \"./nested\" });\n}\n";
    let stage = fixture.stage(&storage, proposed);
    let original = resolve_external_local_project_closure_with_storage(
        stage.requested_root(),
        context.clone(),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve original declaration");
    let candidate = resolve_staged_external_local_project_closure_with_storage(
        &stage,
        context.clone(),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve proposed dependency closure");

    assert_eq!(original.graph().packages().len(), 1);
    assert_eq!(candidate.graph().packages().len(), 4);
    assert_eq!(candidate.root_role(), BuildDeclarationKind::Application);
    assert_eq!(candidate.graph().root(), original.graph().root());
    assert_eq!(
        candidate.source_requests().root().request(),
        original.source_requests().root().request()
    );
    let PackageRootSourceRequest::ExternalLocal {
        requested_root,
        source_context,
    } = candidate.source_requests().root().request()
    else {
        panic!("expected external local root request")
    };
    assert_eq!(
        requested_root.as_os_str(),
        stage.requested_root().as_os_str()
    );
    assert_eq!(source_context, &context);
    assert_eq!(candidate.source_requests().dependencies().count(), 3);
    let root = candidate.custody(candidate.graph().root()).unwrap();
    assert_eq!(root.snapshot_root(), stage.snapshot_root());
    assert_ne!(
        root.resolution(),
        original
            .custody(original.graph().root())
            .unwrap()
            .resolution()
    );
    assert_eq!(
        std::fs::read(root.snapshot_root().join("build.omg")).unwrap(),
        proposed.as_bytes()
    );
    for (name, directory) in [
        ("staged-app", "root"),
        ("middle", "middle"),
        ("leaf", "middle/nested/leaf"),
        ("nested", "root/nested"),
    ] {
        let custody = candidate
            .custodies()
            .iter()
            .find(|custody| custody.key().name().as_str() == name)
            .unwrap();
        assert_eq!(
            custody.key().source_lineage(),
            &SourceLineage::ExternalLocal(
                ExternalLocalLineage::canonicalize(fixture.path(directory), context.clone())
                    .unwrap()
            )
        );
    }
    let exact = candidate.for_exact_target(target::TargetProfile::WindowsX64);
    let subject = CanonicalSourceClosureSubject::from_resolved(
        &exact,
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .expect("canonical staged source graph");
    CanonicalSourceClosureSubject::recover(
        subject.canonical_bytes(),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .expect("recover staged source graph");
    crate::resolution::package_compilation_inputs(&candidate).expect("staged compiler inputs");
    let reviews = compile_resolved_package_candidate_reviews(&exact, &fixture.path("compiler"))
        .expect("check all staged packages through ordinary compiler reviews");
    for custody in candidate.custodies() {
        assert!(reviews.review(custody.key()).is_some());
    }
    assert_eq!(
        std::fs::read(fixture.path("root/build.omg")).unwrap(),
        original_build
    );
    assert_eq!(
        std::fs::read(fixture.path("root/main.omg")).unwrap(),
        original_main
    );
    stage
        .verify_live_source_unchanged()
        .expect("caller still owns unchanged stage");
}

#[test]
fn staged_project_rejects_stale_live_root_before_dependency_acquisition() {
    let fixture = Fixture::new("staged-closure-stale");
    write_package(&fixture.path("root"), "staged-root", None);
    let storage = SourceResolverStorage::for_hardened_base(fixture.path("cache")).unwrap();
    let stage = fixture.stage(&storage, "machine build(builder: &mut Build) { builder.package(\"staged-root\"); builder.depend(Source::Path { location: \"../missing\" }); }\n");
    std::fs::write(fixture.path("root/main.omg"), "machine changed() {}\n").unwrap();
    let error = resolve_staged_external_local_project_closure_with_storage(
        &stage,
        ExternalSourceContext::derive(b"stale-staged-closure"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect_err("live drift rejects before the missing dependency");
    assert!(matches!(
        error,
        ResolveExternalLocalPackageClosureError::Root(ResolvePackageSourceError::Source(
            SourceResolveError::LocalSourceChanged { .. }
        ))
    ));
}

#[test]
fn staged_project_preserves_closure_limits_and_package_only_dependencies() {
    let fixture = Fixture::new("staged-closure-limits");
    write_package(&fixture.path("root"), "staged-root", None);
    write_package(&fixture.path("dependency"), "dependency", None);
    let storage = SourceResolverStorage::for_hardened_base(fixture.path("cache")).unwrap();
    let stage = fixture.stage(&storage, "machine build(builder: &mut Build) { builder.package(\"staged-root\"); builder.depend(Source::Path { location: \"../dependency\" }); }\n");
    let context = ExternalSourceContext::derive(b"staged-closure-limits");
    let error = resolve_staged_external_local_project_closure_with_storage(
        &stage,
        context.clone(),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits {
            max_packages: 1,
            ..PackageSourceClosureLimits::default()
        },
    )
    .expect_err("staged traversal obeys closure limits");
    assert!(matches!(
        error,
        ResolveExternalLocalPackageClosureError::Closure(
            PackageSourceClosureResolutionError::LimitExceeded {
                kind: PackageSourceClosureLimitKind::Packages,
                ..
            }
        )
    ));
    write_application(&fixture.path("dependency"), "dependency", None);
    let error = resolve_staged_external_local_project_closure_with_storage(
        &stage,
        context,
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect_err("dependency may not be an application");
    assert!(matches!(
        error,
        ResolveExternalLocalPackageClosureError::Closure(
            PackageSourceClosureResolutionError::Adapter {
                error: ResolveDependencySourceError::Source(
                    ResolvePackageSourceError::Declaration(_)
                ),
                ..
            }
        )
    ));
}

#[test]
fn staged_pin_policy_keeps_local_lookup_and_rejects_another_root_request() {
    let fixture = Fixture::new("staged-pin-policy");
    write_application(&fixture.path("root"), "staged-app", None);
    write_package(&fixture.path("dependency"), "dependency", None);
    let storage = SourceResolverStorage::for_hardened_base(fixture.path("cache")).unwrap();
    let proposed = "machine build(builder: &mut Build) { builder.application(\"staged-app\"); builder.depend(Source::Path { location: \"../dependency\" }); }\n";
    let stage = fixture.stage(&storage, proposed);
    let context = ExternalSourceContext::derive(b"staged-pin-policy");
    let original = resolve_external_local_project_closure_with_storage(
        stage.requested_root(),
        context.clone(),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .unwrap();
    let subject = CanonicalSourceClosureSubject::from_resolved(
        &original.for_exact_target(target::TargetProfile::WindowsX64),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap();
    let pins = GitDependencyPins::new(&subject, &[], GitExactRevisionAcquisition::Offline).unwrap();
    let candidate = resolve_staged_external_local_project_closure_with_git_pins(
        &stage,
        context,
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
        pins,
    )
    .expect("pin-aware staged resolver still follows relative Path dependencies");
    assert_eq!(candidate.custodies().len(), 2);
    assert_eq!(candidate.graph().root(), original.graph().root());
    assert_eq!(
        candidate
            .custody(candidate.graph().root())
            .unwrap()
            .snapshot_root(),
        stage.snapshot_root()
    );
    let error = resolve_staged_external_local_project_closure_with_git_pins(
        &stage,
        ExternalSourceContext::derive(b"another-context"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
        pins,
    )
    .expect_err("another consuming context cannot supply accepted pins");
    assert!(matches!(
        error,
        ResolveExternalLocalPackageClosureError::RootRequestMismatch
    ));
    write_application(&fixture.path("another"), "staged-app", None);
    let original_build = std::fs::read(fixture.path("another/build.omg")).unwrap();
    let another_stage = stage_local_source_replacement_in_lane(
        &fixture.path("another"),
        &SourceRelativePath::parse("build.omg").unwrap(),
        &Sha256::digest(&original_build).into(),
        proposed.as_bytes(),
        storage.external_local_sources(),
        LocalSourceLimits::default(),
    )
    .unwrap();
    let error = resolve_staged_external_local_project_closure_with_git_pins(
        &another_stage,
        ExternalSourceContext::derive(b"staged-pin-policy"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
        pins,
    )
    .expect_err("identical declaration in another project is not the accepted root request");
    assert!(matches!(
        error,
        ResolveExternalLocalPackageClosureError::RootRequestMismatch
    ));
    stage.verify_live_source_unchanged().unwrap();
    another_stage.verify_live_source_unchanged().unwrap();
}
