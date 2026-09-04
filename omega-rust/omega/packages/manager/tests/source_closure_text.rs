//! The source-graph text record is not an accepted lock or package certificate.

use omega_package_manager::declarations::BuildDeclarationKind;
use omega_package_manager::resolution::graph::{
    CanonicalRootSourceRequest, CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits,
    PackageSourceClosureLimits, ResolvedPackageSourceClosure,
    resolve_external_local_project_closure_with_storage,
    resolve_workspace_package_closure_with_storage,
};
use omega_package_source::{
    ExternalSourceContext, LocalSourceLimits, SourceLineage, SourceRelativePath,
    SourceResolverStorage,
};
use omega_target::TargetProfile;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TempTree(PathBuf);

impl TempTree {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock follows the Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "omega-source-closure-text-{}-{stamp}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).expect("create source-graph text fixture root");
        Self(root)
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        make_tree_owner_writable(&self.0);
        let _ = fs::remove_dir_all(&self.0);
    }
}

// Only the test-owned tree is thawed; Windows readonly snapshot files must be
// writable before deletion, and symlink targets are never traversed.
#[cfg_attr(windows, allow(clippy::permissions_set_readonly_false))]
fn make_tree_owner_writable(root: &Path) {
    let Ok(metadata) = fs::symlink_metadata(root) else {
        return;
    };
    if metadata.file_type().is_symlink() {
        return;
    }
    #[cfg(unix)]
    if metadata.is_dir() {
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode() | 0o700;
        let _ = fs::set_permissions(root, fs::Permissions::from_mode(mode));
    }
    #[cfg(windows)]
    {
        let mut permissions = metadata.permissions();
        permissions.set_readonly(false);
        let _ = fs::set_permissions(root, permissions);
    }
    if metadata.is_dir()
        && let Ok(entries) = fs::read_dir(root)
    {
        for entry in entries.flatten() {
            make_tree_owner_writable(&entry.path());
        }
    }
}

fn write_member(root: &Path, role: &str, name: &str, dependencies: &str) {
    fs::create_dir_all(root).expect("create source member");
    fs::write(
        root.join("build.omg"),
        format!(
            "machine build(builder: &mut Build) {{\n    builder.{role}(\"{name}\");\n{dependencies}}}\n"
        ),
    )
    .expect("write source member declaration");
    fs::write(root.join("main.omg"), "machine value() {}\n").expect("write source member body");
}

fn resolve_diamond(tree: &TempTree, role: &str) -> ResolvedPackageSourceClosure {
    let sources = tree.path("sources");
    write_member(
        &sources.join("root"),
        role,
        "source-text-root",
        concat!(
            "    builder.depend_as(\"left_branch\", Source::Path { location: \"../left\" });\n",
            "    builder.depend_as(\"right_branch\", Source::Path { location: \"../right\" });\n",
        ),
    );
    for (directory, alias) in [("left", "shared_left"), ("right", "shared_right")] {
        write_member(
            &sources.join(directory),
            "package",
            "same-name",
            &format!(
                "    builder.depend_as(\"{alias}\", Source::Path {{ location: \"../shared\" }});\n"
            ),
        );
    }
    write_member(&sources.join("shared"), "package", "shared-package", "");
    let storage = SourceResolverStorage::for_hardened_base(tree.path("cache"))
        .expect("source resolver storage");
    resolve_external_local_project_closure_with_storage(
        sources.join("root"),
        ExternalSourceContext::derive(b"source-closure-text-diamond"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve source diamond with requester-local aliases")
}

fn subject_for(
    closure: &ResolvedPackageSourceClosure,
    target: TargetProfile,
) -> CanonicalSourceClosureSubject {
    CanonicalSourceClosureSubject::from_resolved(
        &closure.for_exact_target(target),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .expect("construct source-graph subject")
}

fn assert_round_trip(subject: &CanonicalSourceClosureSubject) -> String {
    let limits = CanonicalSourceClosureSubjectLimits::default();
    let text = subject
        .canonical_text(limits)
        .expect("encode source graph text");
    let recovered = CanonicalSourceClosureSubject::recover_text(&text, limits)
        .expect("recover source graph text");
    assert_eq!(recovered, *subject);
    assert_eq!(recovered.canonical_bytes(), subject.canonical_bytes());
    assert_eq!(recovered.fingerprint(), subject.fingerprint());
    assert_eq!(recovered.canonical_text(limits).unwrap(), text);
    text
}

#[test]
fn text_preserves_dependency_occurrences_aliases_lineages_roles_and_targets() {
    for (role, expected_role) in [
        ("package", BuildDeclarationKind::Package),
        ("application", BuildDeclarationKind::Application),
    ] {
        let tree = TempTree::new();
        let closure = resolve_diamond(&tree, role);
        let windows = subject_for(&closure, TargetProfile::WindowsX64);
        let linux = subject_for(&closure, TargetProfile::LinuxX64);
        assert_eq!(windows.root_role(), expected_role);
        assert_eq!(windows.packages().len(), 4);
        assert_eq!(windows.dependency_requests().len(), 4);
        let same_named = windows
            .packages()
            .iter()
            .filter(|package| package.key().name().as_str() == "same-name")
            .collect::<Vec<_>>();
        assert_eq!(same_named.len(), 2);
        assert_ne!(same_named[0].key(), same_named[1].key());
        let mut aliases = windows
            .dependency_requests()
            .iter()
            .map(|dependency| dependency.alias().as_str())
            .collect::<Vec<_>>();
        aliases.sort_unstable();
        assert_eq!(
            aliases,
            ["left_branch", "right_branch", "shared_left", "shared_right"]
        );

        let windows_text = assert_round_trip(&windows);
        let linux_text = assert_round_trip(&linux);
        assert_ne!(windows_text, linux_text);
        assert_ne!(windows.fingerprint(), linux.fingerprint());
        assert_eq!(windows.packages(), linux.packages());
        assert_eq!(windows.dependency_requests(), linux.dependency_requests());
    }
}

#[test]
fn workspace_graph_text_recovers_without_old_source_or_cache() {
    let tree = TempTree::new();
    let workspace = tree.path("workspace");
    write_member(
        &workspace.join("root"),
        "package",
        "workspace-root",
        "    builder.depend_as(\"child_alias\", Source::Path { location: \"../child\" });\n",
    );
    write_member(&workspace.join("child"), "package", "workspace-child", "");
    let lineage = SourceLineage::git("https://github.com/CathedralOS/source-text-fixture.git")
        .expect("workspace source lineage");
    let storage = SourceResolverStorage::for_hardened_base(tree.path("cache"))
        .expect("workspace resolver storage");
    let closure = resolve_workspace_package_closure_with_storage(
        &lineage,
        SourceRelativePath::parse("root").unwrap(),
        &workspace,
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve real workspace-member graph");
    let subject = subject_for(&closure, TargetProfile::LinuxX64);
    assert!(matches!(
        subject.root().request(),
        CanonicalRootSourceRequest::WorkspaceMember { .. }
    ));
    let text = assert_round_trip(&subject);
    drop(closure);
    drop(storage);
    fs::remove_dir_all(&workspace).expect("remove old source checkout");
    make_tree_owner_writable(&tree.path("cache"));
    fs::remove_dir_all(tree.path("cache")).expect("remove source cache");

    let recovered = CanonicalSourceClosureSubject::recover_text(
        &text,
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .expect("recover graph without source or compiler replay");
    assert_eq!(recovered.canonical_bytes(), subject.canonical_bytes());
    assert_eq!(recovered.root_role(), BuildDeclarationKind::Package);
    assert_eq!(
        recovered.dependency_requests()[0].alias().as_str(),
        "child_alias"
    );
}

#[test]
fn text_recovery_and_encoding_enforce_resource_bounds() {
    let tree = TempTree::new();
    let closure = resolve_diamond(&tree, "package");
    let subject = subject_for(&closure, TargetProfile::LinuxX64);
    let limits = CanonicalSourceClosureSubjectLimits::default();
    let text = assert_round_trip(&subject);
    for restricted in [
        CanonicalSourceClosureSubjectLimits {
            maximum_record_bytes: text.len() - 1,
            ..limits
        },
        CanonicalSourceClosureSubjectLimits {
            maximum_packages: subject.packages().len() - 1,
            ..limits
        },
        CanonicalSourceClosureSubjectLimits {
            maximum_dependency_requests: subject.dependency_requests().len() - 1,
            ..limits
        },
        CanonicalSourceClosureSubjectLimits {
            maximum_identity_bytes: 1,
            ..limits
        },
        CanonicalSourceClosureSubjectLimits {
            maximum_request_bytes: 1,
            ..limits
        },
    ] {
        assert!(CanonicalSourceClosureSubject::recover_text(&text, restricted).is_err());
        assert!(subject.canonical_text(restricted).is_err());
    }
}

#[test]
fn malformed_unknown_trailing_and_noncanonical_text_rejects() {
    let tree = TempTree::new();
    let closure = resolve_diamond(&tree, "package");
    let subject = subject_for(&closure, TargetProfile::LinuxX64);
    let limits = CanonicalSourceClosureSubjectLimits::default();
    let text = assert_round_trip(&subject);
    let first_line_end = text.find('\n').expect("line-oriented text header");
    assert!(
        text.is_ascii(),
        "canonical text escapes non-ASCII source bytes"
    );
    for length in 0..text.len() {
        assert!(
            CanonicalSourceClosureSubject::recover_text(&text[..length], limits).is_err(),
            "truncated prefix of {length} bytes must reject",
        );
    }
    for malformed in [
        String::new(),
        format!("UNKNOWN-SOURCE-GRAPH 99{}", &text[first_line_end..]),
        text.trim_end_matches('\n').to_owned(),
        text.replace('\n', "\r\n"),
        format!(" {text}"),
        format!("{text}\n"),
        format!("{text}unknown trailing field\n"),
        text.replacen("packages 4\n", "packages 04\n", 1),
        text.replacen(
            "packages 4\n",
            "packages 340282366920938463463374607431768211456\n",
            1,
        ),
        text.replacen("edges 4\n", "edges 04\n", 1),
        text.replacen(
            "edges 4\n",
            "edges 340282366920938463463374607431768211456\n",
            1,
        ),
        text.replacen(
            "name \"source-text-root\"",
            "name \"\\x73ource-text-root\"",
            1,
        ),
    ] {
        assert_ne!(malformed, text);
        assert!(CanonicalSourceClosureSubject::recover_text(&malformed, limits).is_err());
    }
}

#[test]
fn duplicate_and_reordered_complete_package_blocks_reject() {
    let tree = TempTree::new();
    let closure = resolve_diamond(&tree, "package");
    let subject = subject_for(&closure, TargetProfile::LinuxX64);
    let limits = CanonicalSourceClosureSubjectLimits::default();
    let text = assert_round_trip(&subject);
    let (root, remainder) = text
        .split_once("packages 4\n")
        .expect("fixture declares four canonical packages");
    let (package_section, edges) = remainder
        .split_once("edges 4\n")
        .expect("fixture declares four canonical dependency occurrences");
    assert!(package_section.starts_with("package\n"));
    let mut boundaries = vec![0];
    boundaries.extend(
        package_section
            .match_indices("\npackage\n")
            .map(|(offset, _)| offset + 1),
    );
    boundaries.push(package_section.len());
    let packages = boundaries
        .windows(2)
        .map(|pair| &package_section[pair[0]..pair[1]])
        .collect::<Vec<_>>();
    assert_eq!(packages.len(), 4);
    let original = format!("{root}packages 4\n{}edges 4\n{edges}", packages.concat());
    assert_eq!(
        original, text,
        "package-block framing must preserve every field"
    );

    let mut duplicate = packages.clone();
    duplicate[1] = duplicate[0];
    let mut reordered = packages;
    reordered.swap(0, 1);
    for changed in [duplicate, reordered] {
        let malformed = format!("{root}packages 4\n{}edges 4\n{edges}", changed.concat());
        assert_ne!(malformed, text);
        assert!(CanonicalSourceClosureSubject::recover_text(&malformed, limits).is_err());
    }
}

#[test]
fn text_recovery_rejects_unknown_targets_and_inconsistent_graph_semantics() {
    let tree = TempTree::new();
    let closure = resolve_diamond(&tree, "package");
    let subject = subject_for(&closure, TargetProfile::LinuxX64);
    let limits = CanonicalSourceClosureSubjectLimits::default();
    let text = assert_round_trip(&subject);
    let target_line = text
        .lines()
        .find(|line| line.starts_with("target "))
        .expect("explicit target identity line");
    let wrong_alias = text.replacen(
        "resolved-alias \"left_branch\"",
        "resolved-alias \"right_branch\"",
        1,
    );
    let wrong_resolution = text.replacen("resolution external-local ", "resolution workspace ", 1);
    let wrong_count = text.replacen("packages 4\n", "packages 5\n", 1);
    for malformed in [
        text.replacen(target_line, "target \"unknown-target-profile\"", 1),
        wrong_alias,
        wrong_resolution,
        wrong_count,
    ] {
        assert_ne!(
            malformed, text,
            "mutation must alter its intended semantic field"
        );
        assert!(CanonicalSourceClosureSubject::recover_text(&malformed, limits).is_err());
    }
}
