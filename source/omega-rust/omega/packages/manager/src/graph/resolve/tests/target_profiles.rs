//! Production traversal of target-conditioned dependency projections.

use super::*;
use crate::graph::{CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits};
use crate::graph::{PackageSourceClosureResolutionError, ResolvedPackageSourceClosure};
use crate::project::dependencies::read::{ActiveDependencyAliasError, ActiveDependencyAliasScope};
use omega_target::TargetProfile;

fn write_source_package(root: &Path, name: &str) {
    write_package(root, name, None);
}

fn resolve(
    root: &Path,
    profile: TargetProfile,
    cache: &Path,
) -> Result<ResolvedPackageSourceClosure, ResolveExternalLocalPackageClosureError> {
    resolve_external_local_package_closure(
        root,
        ExternalSourceContext::derive(b"target-conditioned-production-traversal"),
        profile,
        cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
}

#[test]
fn selects_common_and_one_exact_profile_without_acquiring_inactive_sources() {
    let sources = temp_root("target-conditioned-sources");
    let cache = temp_root("target-conditioned-cache");
    let root = sources.join("root");
    std::fs::create_dir_all(&root).expect("create root package");
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.package("profile-root");
    builder.depend(Source::Path { location: "../portable" });
    transition builder.target {
        TargetProfile::WindowsX86_64 -> windows(builder)
        TargetProfile::LinuxX86_64 -> linux(builder)
    }

    state windows(builder: &mut Build) {
        builder.depend_as("native_api", Source::Path { location: "../windows" });
    }

    state linux(builder: &mut Build) {
        builder.depend_as("native_api", Source::Path { location: "../missing-linux" });
    }
}
"#,
    )
    .expect("write conditioned root build");
    std::fs::write(root.join("main.omg"), "machine root() {}\n").expect("write root source");
    write_source_package(&sources.join("portable"), "portable-api");
    write_source_package(&sources.join("windows"), "windows-api");

    let closure = resolve(&root, TargetProfile::WindowsX64, &cache)
        .expect("Windows closure must not touch the absent Linux source");
    assert_eq!(closure.target_profile(), TargetProfile::WindowsX64);
    let names = closure
        .custodies()
        .iter()
        .map(|custody| custody.key().name().as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        names,
        std::collections::BTreeSet::from(["portable-api", "profile-root", "windows-api"])
    );

    let root_custody = closure
        .custody(closure.graph().root())
        .expect("root custody");
    assert_eq!(root_custody.dependency_requests().len(), 3);
    assert_eq!(root_custody.projected_dependencies().by_profile().len(), 2);
    let authored_indices = closure
        .source_requests()
        .dependencies()
        .filter(|request| request.requester() == closure.graph().root())
        .map(|request| request.dependency_index())
        .collect::<Vec<_>>();
    assert_eq!(authored_indices, [0, 1]);

    let subject = CanonicalSourceClosureSubject::from_resolved(
        &closure,
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .expect("conditioned closure has canonical identity");
    assert_eq!(subject.target_profile(), TargetProfile::WindowsX64);
    let recovered = CanonicalSourceClosureSubject::recover(
        subject.canonical_bytes(),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .expect("conditioned closure identity recovers");
    let recovered_projection = recovered
        .package_dependency_projection(closure.graph().root())
        .expect("root projection survives recovery");
    assert_eq!(recovered_projection.authored_dependencies().len(), 3);
    assert_eq!(recovered_projection.common_occurrence_indices(), [0]);
    assert_eq!(recovered_projection.by_profile().len(), 2);
    assert_eq!(
        recovered_projection
            .condition_schema()
            .referenced_profile_identities(),
        [
            TargetProfile::LinuxX64.identity(),
            TargetProfile::WindowsX64.identity(),
        ]
    );
    assert!(
        subject
            .matches_resolved(&closure, CanonicalSourceClosureSubjectLimits::default())
            .unwrap()
    );

    let _ = std::fs::remove_dir_all(sources);
    let _ = std::fs::remove_dir_all(cache);
}

#[test]
fn selected_profile_changes_identity_even_when_the_source_graph_does_not() {
    let sources = temp_root("profile-identity-sources");
    let windows_cache = temp_root("profile-identity-windows-cache");
    let linux_cache = temp_root("profile-identity-linux-cache");
    let root = sources.join("root");
    write_source_package(&root, "profile-identity-root");

    let windows = resolve(&root, TargetProfile::WindowsX64, &windows_cache)
        .expect("resolve target-independent package for Windows");
    let linux = resolve(&root, TargetProfile::LinuxX64, &linux_cache)
        .expect("resolve target-independent package for Linux");
    assert_eq!(windows.graph(), linux.graph());

    let windows_subject = CanonicalSourceClosureSubject::from_resolved(
        &windows,
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .expect("Windows subject");
    let linux_subject = CanonicalSourceClosureSubject::from_resolved(
        &linux,
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .expect("Linux subject");
    assert_ne!(
        windows_subject.canonical_bytes(),
        linux_subject.canonical_bytes()
    );
    assert_ne!(windows_subject.fingerprint(), linux_subject.fingerprint());

    let _ = std::fs::remove_dir_all(sources);
    let _ = std::fs::remove_dir_all(windows_cache);
    let _ = std::fs::remove_dir_all(linux_cache);
}

#[test]
fn permits_alias_reuse_only_between_inactive_profile_columns() {
    let sources = temp_root("exclusive-alias-sources");
    let windows_cache = temp_root("exclusive-alias-windows-cache");
    let linux_cache = temp_root("exclusive-alias-linux-cache");
    let root = sources.join("root");
    std::fs::create_dir_all(&root).expect("create root package");
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.package("exclusive-alias-root");
    transition builder.target {
        TargetProfile::WindowsX86_64 -> windows(builder)
        TargetProfile::LinuxX86_64 -> linux(builder)
    }

    state windows(builder: &mut Build) {
        builder.depend_as("native_api", Source::Path { location: "../windows" });
    }

    state linux(builder: &mut Build) {
        builder.depend_as("native_api", Source::Path { location: "../linux" });
    }
}
"#,
    )
    .expect("write mutually exclusive aliases");
    std::fs::write(root.join("main.omg"), "machine root() {}\n").expect("write root source");
    write_source_package(&sources.join("windows"), "windows-api");
    write_source_package(&sources.join("linux"), "linux-api");

    let windows =
        resolve(&root, TargetProfile::WindowsX64, &windows_cache).expect("resolve Windows alias");
    let linux = resolve(&root, TargetProfile::LinuxX64, &linux_cache).expect("resolve Linux alias");
    for closure in [&windows, &linux] {
        let dependencies = closure
            .graph()
            .package(closure.graph().root())
            .expect("root node")
            .dependencies();
        assert_eq!(dependencies.len(), 1);
        assert_eq!(dependencies[0].alias().as_str(), "native_api");
    }

    let _ = std::fs::remove_dir_all(sources);
    let _ = std::fs::remove_dir_all(windows_cache);
    let _ = std::fs::remove_dir_all(linux_cache);
}

#[test]
fn rejects_an_active_alias_conflict_before_emitting_requester_edges() {
    let sources = temp_root("active-alias-conflict-sources");
    let windows_cache = temp_root("active-alias-conflict-windows-cache");
    let linux_cache = temp_root("active-alias-conflict-linux-cache");
    let root = sources.join("root");
    std::fs::create_dir_all(&root).expect("create root package");
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.package("active-conflict-root");
    builder.depend_as("native_api", Source::Path { location: "../portable" });
    transition builder.target {
        TargetProfile::WindowsX86_64 -> windows(builder)
    }

    state windows(builder: &mut Build) {
        builder.depend_as("native_api", Source::Path { location: "../windows" });
    }
}
"#,
    )
    .expect("write active alias conflict");
    std::fs::write(root.join("main.omg"), "machine root() {}\n").expect("write root source");
    write_source_package(&sources.join("portable"), "portable-api");
    write_source_package(&sources.join("windows"), "windows-api");

    let error = resolve(&root, TargetProfile::WindowsX64, &windows_cache)
        .expect_err("common and selected profile aliases must conflict");
    assert!(matches!(
        error,
        ResolveExternalLocalPackageClosureError::Closure(
            PackageSourceClosureResolutionError::InvalidActiveAliases {
                error: ActiveDependencyAliasError::DuplicateAlias {
                    scope: ActiveDependencyAliasScope::Profile(TargetProfile::WindowsX64),
                    ref alias,
                    ..
                },
                ..
            }
        ) if alias.as_str() == "native_api"
    ));

    let portable = resolve(&root, TargetProfile::LinuxX64, &linux_cache)
        .expect("an unreferenced exact profile selects the common column only");
    assert_eq!(
        portable
            .graph()
            .package(portable.graph().root())
            .expect("root node")
            .dependencies()
            .len(),
        1
    );

    let _ = std::fs::remove_dir_all(sources);
    let _ = std::fs::remove_dir_all(windows_cache);
    let _ = std::fs::remove_dir_all(linux_cache);
}
