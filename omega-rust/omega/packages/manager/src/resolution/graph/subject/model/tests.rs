use super::*;
use crate::declarations::BuildDeclarationKind;
use crate::declarations::{AliasName, PackageKey, PackageName};
use crate::resolution::graph::ResolvedSourceIdentity;
use crate::resolution::source::PackageSourceNavigation;
use package_source::{
    GitCommitId, GitTreeId, ImmutableSourceResolution, SourceLineage, SourceRelativePath,
};

fn finish(
    root: CanonicalRootSourceSelection,
    packages: Vec<ResolvedSourceIdentity>,
    dependency_requests: Vec<CanonicalDependencySourceSelection>,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectError> {
    let navigations = vec![PackageSourceNavigation::Root; packages.len()];
    CanonicalSourceClosureSubject::finish(root, packages, navigations, dependency_requests, limits)
}

fn git_source(name: &str, repository: &str, marker: u8) -> ResolvedSourceIdentity {
    let key = PackageKey::new(
        PackageName::parse(name).unwrap(),
        SourceLineage::git(&format!("https://github.com/CathedralOS/{repository}.git")).unwrap(),
    );
    let digit = char::from_digit(u32::from(marker % 10), 16).unwrap();
    let next = char::from_digit(u32::from((marker + 1) % 10), 16).unwrap();
    let resolution = ImmutableSourceResolution::git(
        GitCommitId::parse_hex(&digit.to_string().repeat(40)).unwrap(),
        GitTreeId::parse_hex(&next.to_string().repeat(40)).unwrap(),
    )
    .unwrap();
    ResolvedSourceIdentity::new(key, resolution).unwrap()
}

fn root_git_selection(
    locator: &str,
    selected: &ResolvedSourceIdentity,
) -> CanonicalRootSourceSelection {
    CanonicalRootSourceSelection {
        request: CanonicalRootSourceRequest::Git {
            requested_locator: locator.to_owned(),
            requested_revision: "main".to_owned(),
            selection: crate::declarations::PackageSelection::Root,
        },
        role: BuildDeclarationKind::Package,
        selected: selected.clone(),
    }
}

#[test]
fn borrowed_source_graph_comparison_excludes_only_target_and_derived_encoding() {
    let source = git_source("codec", "codec", 1);
    let original = finish(
        root_git_selection("https://github.com/CathedralOS/codec.git", &source),
        vec![source],
        Vec::new(),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap();
    let mut changed = original.clone();
    changed.target_profile = target::TargetProfile::WindowsX64;
    changed.canonical_bytes.clear();
    changed.fingerprint = super::super::encoding::fingerprint(b"different target bytes");
    assert!(original.same_source_graph(&changed));

    // Diagnostic-only copies exercise each stored source family independently;
    // no edited copy is represented as a validated recovered subject.
    changed = original.clone();
    changed.root.role = BuildDeclarationKind::Application;
    assert!(!original.same_source_graph(&changed));
    changed = original.clone();
    changed.packages.clear();
    assert!(!original.same_source_graph(&changed));
    changed = original.clone();
    changed.package_navigations.clear();
    assert!(!original.same_source_graph(&changed));
    changed = original.clone();
    changed.package_dependency_projections.clear();
    assert!(!original.same_source_graph(&changed));
    changed = original.clone();
    changed
        .dependency_requests
        .push(CanonicalDependencySourceSelection {
            requester: original.packages[0].key().clone(),
            dependency_index: 0,
            request: CanonicalDependencySourceRequest::Path {
                explicit_alias: None,
                location: "../child".to_owned(),
            },
            alias: AliasName::parse("child").unwrap(),
            selected: original.packages[0].clone(),
        });
    assert!(!original.same_source_graph(&changed));
}

#[test]
fn readable_source_subject_preserves_git_lineages_object_formats_and_targets() {
    let limits = CanonicalSourceClosureSubjectLimits::default();
    for locator in [
        "https://github.com/CathedralOS/codec.git",
        "git@github.com:CathedralOS/codec.git",
        "https://gitlab.com/team/subgroup/codec.git",
        "https://git.example.org:8443/team/codec.git",
        "ssh://builder@git.example.org:2222/team/codec.git",
        "builder@git.example.org:team/codec.git",
    ] {
        for object_digits in [40, 64] {
            let source = ResolvedSourceIdentity::new(
                PackageKey::new(
                    PackageName::parse("codec").unwrap(),
                    SourceLineage::git(locator).unwrap(),
                ),
                ImmutableSourceResolution::git(
                    GitCommitId::parse_hex(&"1".repeat(object_digits)).unwrap(),
                    GitTreeId::parse_hex(&"2".repeat(object_digits)).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            for target in target::TargetProfile::ALL {
                let original = CanonicalSourceClosureSubject::finish_for_target(
                    target,
                    root_git_selection(locator, &source),
                    vec![source.clone()],
                    vec![PackageSourceNavigation::Root],
                    Vec::new(),
                    limits,
                )
                .unwrap();
                let text = original.canonical_text(limits).unwrap();
                assert!(
                    text.contains(locator),
                    "exact requested locator stays readable"
                );
                assert!(text.contains("codec"));
                assert!(text.contains(&"1".repeat(object_digits)));
                assert!(text.contains(target.identity().as_str()));
                let recovered = CanonicalSourceClosureSubject::recover_text(&text, limits)
                    .expect("source text supports every admitted Git lineage and target");
                assert_eq!(recovered, original);
                assert_eq!(recovered.canonical_bytes(), original.canonical_bytes());
                assert_eq!(recovered.canonical_text(limits).unwrap(), text);
                let (bounded, usage) = CanonicalSourceClosureSubject::recover_text_with_usage(
                    &text,
                    limits,
                    usize::MAX,
                )
                .unwrap();
                assert_eq!(bounded, original);
                assert_eq!(usage.packages(), 1);
                assert_eq!(usage.dependency_requests(), 0);
                assert_eq!(
                    CanonicalSourceClosureSubject::recover_text_with_usage(
                        &text,
                        limits,
                        usage.owned_bytes(),
                    )
                    .unwrap(),
                    (bounded, usage),
                );
                assert!(
                    CanonicalSourceClosureSubject::recover_text_with_usage(
                        &text,
                        limits,
                        usage.owned_bytes() - 1,
                    )
                    .is_err()
                );
            }
        }
    }
}

#[test]
fn readable_source_subject_preserves_named_git_member_requests_and_aliases() {
    let limits = CanonicalSourceClosureSubjectLimits::default();
    let root = git_source("root", "workspace", 1);
    let child = git_source("child", "workspace", 1);
    let root_request = CanonicalRootSourceSelection {
        request: CanonicalRootSourceRequest::Git {
            requested_locator: "https://github.com/CathedralOS/workspace.git".to_owned(),
            requested_revision: "main".to_owned(),
            selection: crate::declarations::PackageSelection::Named(root.key().name().clone()),
        },
        role: BuildDeclarationKind::Application,
        selected: root.clone(),
    };
    let alias = AliasName::parse("codec_alias").unwrap();
    let selected_dependency = CanonicalDependencySourceSelection {
        requester: root.key().clone(),
        dependency_index: 0,
        request: CanonicalDependencySourceRequest::Git {
            explicit_alias: Some(alias.clone()),
            repository: "https://github.com/CathedralOS/workspace.git".to_owned(),
            revision: "main".to_owned(),
            selection: crate::declarations::PackageSelection::Named(child.key().name().clone()),
        },
        alias,
        selected: child.clone(),
    };
    let original = CanonicalSourceClosureSubject::finish(
        root_request,
        vec![child, root],
        vec![
            PackageSourceNavigation::Member(SourceRelativePath::parse("libs/codec").unwrap()),
            PackageSourceNavigation::Member(SourceRelativePath::parse("apps/main").unwrap()),
        ],
        vec![selected_dependency],
        limits,
    )
    .unwrap();
    let text = original.canonical_text(limits).unwrap();
    for spelling in ["codec_alias", "libs/codec", "apps/main"] {
        assert!(text.contains(spelling));
    }
    assert_eq!(
        CanonicalSourceClosureSubject::recover_text(&text, limits).unwrap(),
        original
    );
}

#[test]
fn readable_source_subject_preserves_platform_request_bytes_without_loss() {
    let limits = CanonicalSourceClosureSubjectLimits::default();
    let workspace_root_source =
        SourceLineage::git("https://github.com/CathedralOS/byte-paths.git").unwrap();
    let member_path = SourceRelativePath::parse("member").unwrap();
    let identity =
        package_source::WorkspaceLineageIdentity::from_root_source(&workspace_root_source).unwrap();
    let source = ResolvedSourceIdentity::new(
        PackageKey::new(
            PackageName::parse("byte-paths").unwrap(),
            SourceLineage::Workspace(package_source::WorkspaceMemberLineage::new(
                identity,
                member_path.clone(),
            )),
        ),
        ImmutableSourceResolution::workspace(
            package_source::SourceContentDigest::parse_hex(&"1".repeat(64)).unwrap(),
        ),
    )
    .unwrap();
    // The subject retains platform-encoded request custody as bytes. Exercise
    // the whole byte alphabet without asking a host filesystem to interpret it.
    let original = finish(
        CanonicalRootSourceSelection {
            request: CanonicalRootSourceRequest::WorkspaceMember {
                workspace_root_source,
                member_path,
                requested_workspace_root: (0..=u8::MAX).collect(),
            },
            role: BuildDeclarationKind::Package,
            selected: source.clone(),
        },
        vec![source],
        Vec::new(),
        limits,
    )
    .unwrap();
    let text = original.canonical_text(limits).unwrap();
    assert!(text.is_ascii());
    assert!(text.contains("\\x00"));
    assert!(text.contains("\\xff"));
    assert_eq!(
        CanonicalSourceClosureSubject::recover_text(&text, limits).unwrap(),
        original
    );
    for malformed in [
        text.replacen("\\xff", "\\xFF", 1),
        text.replacen("\\xff", "\\xfg", 1),
        text.replacen("\\xff", "\\qff", 1),
    ] {
        assert_ne!(malformed, text);
        assert!(CanonicalSourceClosureSubject::recover_text(&malformed, limits).is_err());
    }
}

#[test]
fn exact_git_request_spelling_changes_subject_without_changing_selection() {
    let selected = git_source("codec", "codec", 1);
    let https = finish(
        root_git_selection("https://github.com/CathedralOS/codec.git", &selected),
        vec![selected.clone()],
        Vec::new(),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap();
    let ssh = finish(
        root_git_selection("git@github.com:CathedralOS/codec.git", &selected),
        vec![selected],
        Vec::new(),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap();

    assert_eq!(https.root.selected, ssh.root.selected);
    assert_ne!(https.canonical_bytes, ssh.canonical_bytes);
    assert_ne!(https.fingerprint, ssh.fingerprint);
    assert_eq!(
        CanonicalSourceClosureSubject::recover(
            https.canonical_bytes(),
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .unwrap(),
        https
    );
}

#[test]
fn root_role_is_canonical_identity_and_survives_recovery() {
    let selected = git_source("codec", "codec", 1);
    let package = finish(
        root_git_selection("https://github.com/CathedralOS/codec.git", &selected),
        vec![selected.clone()],
        Vec::new(),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .expect("package root subject");
    let mut application_root =
        root_git_selection("https://github.com/CathedralOS/codec.git", &selected);
    application_root.role = BuildDeclarationKind::Application;
    let application = finish(
        application_root,
        vec![selected],
        Vec::new(),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .expect("application root subject");

    assert_eq!(package.root_role(), BuildDeclarationKind::Package);
    assert_eq!(application.root_role(), BuildDeclarationKind::Application);
    assert_ne!(package.canonical_bytes(), application.canonical_bytes());
    assert_ne!(package.fingerprint(), application.fingerprint());
    assert_eq!(
        CanonicalSourceClosureSubject::recover(
            application.canonical_bytes(),
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .expect("recover application role")
        .root_role(),
        BuildDeclarationKind::Application
    );
}

#[test]
fn target_profile_is_canonical_invocation_identity_and_survives_recovery() {
    let selected = git_source("codec", "codec", 1);
    let subject = |target_profile| {
        CanonicalSourceClosureSubject::finish_for_target(
            target_profile,
            root_git_selection("https://github.com/CathedralOS/codec.git", &selected),
            vec![selected.clone()],
            vec![PackageSourceNavigation::Root],
            Vec::new(),
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .expect("target-specific subject")
    };
    let linux = subject(target::TargetProfile::LinuxX64);
    let windows = subject(target::TargetProfile::WindowsX64);

    assert_eq!(linux.packages(), windows.packages());
    assert_ne!(linux.canonical_bytes(), windows.canonical_bytes());
    assert_ne!(linux.fingerprint(), windows.fingerprint());
    assert_eq!(
        CanonicalSourceClosureSubject::recover(
            windows.canonical_bytes(),
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .expect("recover target-specific subject")
        .target_profile(),
        target::TargetProfile::WindowsX64
    );
}

#[test]
fn root_git_package_selection_and_navigation_change_the_subject() {
    let selected = git_source("matrix", "workspace", 1);
    let subject = |selection, navigation| {
        CanonicalSourceClosureSubject::finish(
            CanonicalRootSourceSelection {
                request: CanonicalRootSourceRequest::Git {
                    requested_locator: "https://github.com/CathedralOS/workspace.git".to_owned(),
                    requested_revision: "main".to_owned(),
                    selection,
                },
                role: BuildDeclarationKind::Package,
                selected: selected.clone(),
            },
            vec![selected.clone()],
            vec![navigation],
            Vec::new(),
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .expect("canonical root selection")
    };
    let repository_root = subject(
        crate::declarations::PackageSelection::Root,
        PackageSourceNavigation::Root,
    );
    let named_member = subject(
        crate::declarations::PackageSelection::Named(PackageName::parse("matrix").unwrap()),
        PackageSourceNavigation::Member(SourceRelativePath::parse("packages/matrix").unwrap()),
    );

    assert_eq!(repository_root.packages(), named_member.packages());
    assert_ne!(
        repository_root.canonical_bytes(),
        named_member.canonical_bytes()
    );
    assert_ne!(repository_root.fingerprint(), named_member.fingerprint());
    assert_eq!(
        CanonicalSourceClosureSubject::recover(
            named_member.canonical_bytes(),
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .unwrap(),
        named_member
    );
}

#[test]
fn git_package_selection_is_canonical_request_custody_not_package_identity() {
    let root = git_source("root", "workspace", 1);
    let child = git_source("child", "workspace", 1);
    let request = |selection| CanonicalDependencySourceSelection {
        requester: root.key().clone(),
        dependency_index: 0,
        request: CanonicalDependencySourceRequest::Git {
            explicit_alias: None,
            repository: "https://github.com/CathedralOS/workspace.git".to_owned(),
            revision: "main".to_owned(),
            selection,
        },
        alias: child.key().name().default_alias(),
        selected: child.clone(),
    };
    let subject = |selection| {
        let navigations = match &selection {
            crate::declarations::PackageSelection::Root => {
                vec![PackageSourceNavigation::Root, PackageSourceNavigation::Root]
            }
            crate::declarations::PackageSelection::Named(_) => vec![
                PackageSourceNavigation::Member(SourceRelativePath::parse("child").unwrap()),
                PackageSourceNavigation::Root,
            ],
        };
        CanonicalSourceClosureSubject::finish(
            root_git_selection("https://github.com/CathedralOS/workspace.git", &root),
            vec![child.clone(), root.clone()],
            navigations,
            vec![request(selection)],
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .unwrap()
    };
    let root_selected = subject(crate::declarations::PackageSelection::Root);
    let named_selected = subject(crate::declarations::PackageSelection::Named(
        PackageName::parse("child").unwrap(),
    ));

    assert_eq!(root_selected.packages, named_selected.packages);
    assert_ne!(
        root_selected.canonical_bytes,
        named_selected.canonical_bytes
    );
    assert_eq!(
        CanonicalSourceClosureSubject::recover(
            named_selected.canonical_bytes(),
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .unwrap(),
        named_selected
    );

    let mismatch = CanonicalSourceClosureSubject::finish(
        root_git_selection("https://github.com/CathedralOS/workspace.git", &root),
        vec![child.clone(), root.clone()],
        vec![
            PackageSourceNavigation::Member(SourceRelativePath::parse("child").unwrap()),
            PackageSourceNavigation::Root,
        ],
        vec![request(crate::declarations::PackageSelection::Named(
            PackageName::parse("other").unwrap(),
        ))],
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        mismatch.message(),
        "named Git package selection disagrees with its selected package"
    );
}

#[test]
fn request_and_edge_disagreement_reject_before_encoding() {
    let root = git_source("root", "root", 1);
    let child = git_source("child", "child", 2);
    let request = CanonicalDependencySourceSelection {
        requester: root.key().clone(),
        dependency_index: 0,
        request: CanonicalDependencySourceRequest::Git {
            explicit_alias: None,
            repository: "https://github.com/CathedralOS/child.git".to_owned(),
            revision: "main".to_owned(),
            selection: crate::declarations::PackageSelection::Root,
        },
        alias: AliasName::parse("wrong_alias").unwrap(),
        selected: child.clone(),
    };
    let error = finish(
        root_git_selection("https://github.com/CathedralOS/root.git", &root),
        vec![child, root],
        vec![request],
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.message(),
        "dependency request alias disagrees with its authored selection"
    );
}

#[test]
fn missing_ordinals_and_open_graphs_reject() {
    let root = git_source("root", "root", 1);
    let child = git_source("child", "child", 2);
    let request = |dependency_index| CanonicalDependencySourceSelection {
        requester: root.key().clone(),
        dependency_index,
        request: CanonicalDependencySourceRequest::Git {
            explicit_alias: None,
            repository: "https://github.com/CathedralOS/child.git".to_owned(),
            revision: "main".to_owned(),
            selection: crate::declarations::PackageSelection::Root,
        },
        alias: child.key().name().default_alias(),
        selected: child.clone(),
    };
    let error = finish(
        root_git_selection("https://github.com/CathedralOS/root.git", &root),
        vec![child.clone(), root.clone()],
        vec![request(1)],
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.message(),
        "dependency request ordinals do not begin at zero"
    );

    let error = finish(
        root_git_selection("https://github.com/CathedralOS/root.git", &root),
        vec![root.clone()],
        vec![request(0)],
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.message(),
        "dependency request selection is absent or resolution-mismatched"
    );
}

#[test]
fn recovery_rejects_unknown_version_trailing_bytes_and_tight_limits() {
    let selected = git_source("codec", "codec", 1);
    let subject = finish(
        root_git_selection("https://github.com/CathedralOS/codec.git", &selected),
        vec![selected],
        Vec::new(),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap();

    let mut unknown_version = subject.canonical_bytes.clone();
    let version_offset = SOURCE_CLOSURE_SUBJECT_MAGIC.len();
    unknown_version[version_offset..version_offset + 2]
        .copy_from_slice(&(SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION + 1).to_le_bytes());
    assert!(
        CanonicalSourceClosureSubject::recover(
            &unknown_version,
            CanonicalSourceClosureSubjectLimits::default()
        )
        .is_err()
    );

    let mut retired_conditional_version = subject.canonical_bytes.clone();
    retired_conditional_version[version_offset..version_offset + 2]
        .copy_from_slice(&(SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION - 1).to_le_bytes());
    assert!(
        CanonicalSourceClosureSubject::recover(
            &retired_conditional_version,
            CanonicalSourceClosureSubjectLimits::default()
        )
        .is_err()
    );

    let mut trailing = subject.canonical_bytes.clone();
    trailing.push(0);
    assert!(
        CanonicalSourceClosureSubject::recover(
            &trailing,
            CanonicalSourceClosureSubjectLimits::default()
        )
        .is_err()
    );

    let limits = CanonicalSourceClosureSubjectLimits {
        maximum_record_bytes: subject.canonical_bytes.len() - 1,
        ..CanonicalSourceClosureSubjectLimits::default()
    };
    assert!(CanonicalSourceClosureSubject::recover(subject.canonical_bytes(), limits).is_err());
}

#[test]
fn noncanonical_unreachable_and_cyclic_package_state_rejects() {
    let root = git_source("root", "root", 1);
    let child = git_source("child", "child", 2);
    let root_selection = root_git_selection("https://github.com/CathedralOS/root.git", &root);

    let error = finish(
        root_selection.clone(),
        vec![root.clone(), child.clone()],
        Vec::new(),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.message(),
        "source-closure packages are not in strict canonical order"
    );

    let error = finish(
        root_selection.clone(),
        vec![child.clone(), root.clone()],
        Vec::new(),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.message(),
        "source-closure subject does not form one closed reachable acyclic graph"
    );

    let request = |requester: &ResolvedSourceIdentity,
                   selected: &ResolvedSourceIdentity,
                   repository: &str| CanonicalDependencySourceSelection {
        requester: requester.key().clone(),
        dependency_index: 0,
        request: CanonicalDependencySourceRequest::Git {
            explicit_alias: None,
            repository: repository.to_owned(),
            revision: "main".to_owned(),
            selection: crate::declarations::PackageSelection::Root,
        },
        alias: selected.key().name().default_alias(),
        selected: selected.clone(),
    };
    let error = finish(
        root_selection,
        vec![child.clone(), root.clone()],
        vec![
            request(&child, &root, "https://github.com/CathedralOS/root.git"),
            request(&root, &child, "https://github.com/CathedralOS/child.git"),
        ],
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.message(),
        "source-closure subject does not form one closed reachable acyclic graph"
    );
}
