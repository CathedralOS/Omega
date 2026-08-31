//! The selected target remains invocation identity, never dependency selection.

use super::*;
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits,
    ResolvedPackageSourceClosure,
};
use omega_target::TargetProfile;

fn resolve(
    root: &Path,
    profile: TargetProfile,
    cache: &Path,
) -> Result<ResolvedPackageSourceClosure, ResolveExternalLocalPackageClosureError> {
    resolve_external_local_package_closure(
        root,
        ExternalSourceContext::derive(b"flat-dependency-target-identity"),
        profile,
        cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
}

#[test]
fn selected_profile_changes_identity_even_when_the_source_graph_does_not() {
    let sources = temp_root("profile-identity-sources");
    let windows_cache = temp_root("profile-identity-windows-cache");
    let linux_cache = temp_root("profile-identity-linux-cache");
    let root = sources.join("root");
    write_package(&root, "profile-identity-root", Some("../dependency"));
    write_package(
        &sources.join("dependency"),
        "profile-independent-dependency",
        None,
    );

    let windows = resolve(&root, TargetProfile::WindowsX64, &windows_cache)
        .expect("resolve flat package graph for Windows");
    let linux = resolve(&root, TargetProfile::LinuxX64, &linux_cache)
        .expect("resolve flat package graph for Linux");
    assert_eq!(windows.graph(), linux.graph());
    assert_eq!(
        windows.source_requests().dependencies().count(),
        linux.source_requests().dependencies().count()
    );

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
