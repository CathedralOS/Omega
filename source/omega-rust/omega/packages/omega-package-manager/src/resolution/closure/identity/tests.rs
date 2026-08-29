use super::*;
use omega_package_source::{
    GitCommitId, GitTreeId, ImmutableSourceResolution, SourceContentDigest,
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
        SourceContentDigest::derive(&[marker]),
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
            selection: crate::manifest::PackageSelection::Root,
        },
        selected: selected.clone(),
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
            crate::manifest::PackageSelection::Root => {
                vec![PackageSourceNavigation::Root, PackageSourceNavigation::Root]
            }
            crate::manifest::PackageSelection::Named(_) => vec![
                PackageSourceNavigation::Member(WorkspaceMemberPath::parse("child").unwrap()),
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
    let root_selected = subject(crate::manifest::PackageSelection::Root);
    let named_selected = subject(crate::manifest::PackageSelection::Named(
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
            PackageSourceNavigation::Member(WorkspaceMemberPath::parse("child").unwrap()),
            PackageSourceNavigation::Root,
        ],
        vec![request(crate::manifest::PackageSelection::Named(
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
            selection: crate::manifest::PackageSelection::Root,
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
            selection: crate::manifest::PackageSelection::Root,
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
            selection: crate::manifest::PackageSelection::Root,
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
