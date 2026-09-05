//! The selected target remains invocation identity, never dependency selection.

use super::*;
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits,
    ResolvedPackageSourceClosure,
};
use target::TargetProfile;

fn resolve(
    root: &Path,
    cache: &Path,
) -> Result<ResolvedPackageSourceClosure, ResolveExternalLocalPackageClosureError> {
    resolve_external_local_package_closure(
        root,
        ExternalSourceContext::derive(b"flat-dependency-target-identity"),
        cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
}

#[test]
fn selected_profile_changes_identity_even_when_the_source_graph_does_not() {
    let sources = temp_root("profile-identity-sources");
    let cache = temp_root("profile-identity-cache");
    let root = sources.join("root");
    write_package(&root, "profile-identity-root", Some("../dependency"));
    write_package(
        &sources.join("dependency"),
        "profile-independent-dependency",
        None,
    );

    let closure = resolve(&root, &cache).expect("resolve flat package graph once");
    let windows = closure.for_exact_target(TargetProfile::WindowsX64);
    let linux = closure.for_exact_target(TargetProfile::LinuxX64);
    assert_eq!(windows.target_profile(), TargetProfile::WindowsX64);
    assert_eq!(linux.target_profile(), TargetProfile::LinuxX64);
    assert!(std::ptr::eq(
        windows.source_closure(),
        linux.source_closure()
    ));
    assert_eq!(
        windows.source_closure().graph(),
        linux.source_closure().graph()
    );
    assert_eq!(
        windows
            .source_closure()
            .source_requests()
            .dependencies()
            .count(),
        linux
            .source_closure()
            .source_requests()
            .dependencies()
            .count()
    );

    let windows_subject = CanonicalSourceClosureSubject::from_resolved(
        &windows,
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .expect("Windows subject");
    let windows_bytes = windows_subject.canonical_bytes().to_vec();
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
    assert_eq!(windows_subject.canonical_bytes(), windows_bytes);
    assert_eq!(
        CanonicalSourceClosureSubject::recover(
            &windows_bytes,
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .expect("recover Windows child subject")
        .target_profile(),
        TargetProfile::WindowsX64,
    );

    let _ = std::fs::remove_dir_all(sources);
    let _ = std::fs::remove_dir_all(cache);
}
