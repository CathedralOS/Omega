use super::*;
use crate::manifest::PackageName;
use crate::manifest::{AliasName, PackageKey};
use omega_package_source::ImmutableSourceResolution;
use omega_package_source::{GitCommitId, GitTreeId, SourceLineage};

fn key(name: &str, repository: &str) -> PackageKey {
    PackageKey::new(
        PackageName::parse(name).expect("valid test package name"),
        SourceLineage::git(&format!("https://github.com/CathedralOS/{repository}.git"))
            .expect("valid test source lineage"),
    )
}

fn resolution(marker: u8) -> ImmutableSourceResolution {
    let commit_digit = char::from_digit(u32::from(marker % 10), 16).unwrap();
    let tree_digit = char::from_digit(u32::from((marker + 1) % 10), 16).unwrap();
    ImmutableSourceResolution::git(
        GitCommitId::parse_hex(&commit_digit.to_string().repeat(40)).unwrap(),
        GitTreeId::parse_hex(&tree_digit.to_string().repeat(40)).unwrap(),
    )
    .unwrap()
}

fn alias(value: &str) -> AliasName {
    AliasName::parse(value).expect("valid test alias")
}

fn dependency(alias_name: &str, target: &PackageKey) -> ResolvedDependency {
    ResolvedDependency::new(alias(alias_name), target.clone())
}

fn node(
    key: &PackageKey,
    marker: u8,
    dependencies: Vec<ResolvedDependency>,
) -> ResolvedPackageNode {
    ResolvedPackageNode::new(
        ResolvedSourceIdentity::new(key.clone(), resolution(marker)).unwrap(),
        dependencies,
    )
}

#[test]
fn same_name_from_different_lineages_remains_distinct() {
    let root = key("application", "application");
    let first = key("codec", "codec-one");
    let second = key("codec", "codec-two");

    let closure = ResolvedPackageClosure::new(
        root.clone(),
        crate::manifest::BuildDeclarationKind::Package,
        vec![
            node(
                &root,
                1,
                vec![
                    dependency("codec_one", &first),
                    dependency("codec_two", &second),
                ],
            ),
            node(&first, 2, vec![]),
            node(&second, 3, vec![]),
        ],
    )
    .unwrap();

    assert_ne!(first, second);
    assert!(closure.package(&first).is_some());
    assert!(closure.package(&second).is_some());
}

#[test]
fn same_name_wrong_lineage_does_not_satisfy_an_edge() {
    let root = key("application", "application");
    let requested = key("codec", "expected-codec");
    let spoof = key("codec", "spoof-codec");

    let errors = ResolvedPackageClosure::new(
        root.clone(),
        crate::manifest::BuildDeclarationKind::Package,
        vec![
            node(&root, 1, vec![dependency("codec", &requested)]),
            node(&spoof, 2, vec![]),
        ],
    )
    .unwrap_err();

    assert!(errors.iter().any(|error| matches!(
        error,
        PackageClosureValidationError::MissingDependencyTarget { target, .. }
            if target == &requested
    )));
    assert!(errors.iter().any(|error| matches!(
        error,
        PackageClosureValidationError::UnreachablePackage { key } if key == &spoof
    )));
}

#[test]
fn conflicting_resolutions_for_one_key_are_rejected() {
    let root = key("application", "application");
    let errors = ResolvedPackageClosure::new(
        root.clone(),
        crate::manifest::BuildDeclarationKind::Package,
        vec![node(&root, 1, vec![]), node(&root, 2, vec![])],
    )
    .unwrap_err();

    assert!(errors.iter().any(|error| matches!(
        error,
        PackageClosureValidationError::ConflictingResolution { key, .. } if key == &root
    )));
}

#[test]
fn duplicate_identical_package_rows_are_rejected() {
    let root = key("application", "application");
    let errors = ResolvedPackageClosure::new(
        root.clone(),
        crate::manifest::BuildDeclarationKind::Package,
        vec![node(&root, 1, vec![]), node(&root, 1, vec![])],
    )
    .unwrap_err();

    assert!(errors.iter().any(|error| matches!(
        error,
        PackageClosureValidationError::DuplicatePackage { key } if key == &root
    )));
}

#[test]
fn root_requires_the_exact_package_key() {
    let requested_root = key("application", "expected-application");
    let same_name_wrong_lineage = key("application", "other-application");
    let errors = ResolvedPackageClosure::new(
        requested_root.clone(),
        crate::manifest::BuildDeclarationKind::Package,
        vec![node(&same_name_wrong_lineage, 1, vec![])],
    )
    .unwrap_err();

    assert!(errors.iter().any(|error| matches!(
        error,
        PackageClosureValidationError::MissingRoot { root } if root == &requested_root
    )));
}

#[test]
fn closed_diamond_succeeds() {
    let root = key("application", "application");
    let left = key("left", "left");
    let right = key("right", "right");
    let leaf = key("leaf", "leaf");

    let closure = ResolvedPackageClosure::new(
        root.clone(),
        crate::manifest::BuildDeclarationKind::Package,
        vec![
            node(
                &root,
                1,
                vec![dependency("left", &left), dependency("right", &right)],
            ),
            node(&left, 2, vec![dependency("leaf", &leaf)]),
            node(&right, 3, vec![dependency("leaf", &leaf)]),
            node(&leaf, 4, vec![]),
        ],
    )
    .unwrap();

    assert_eq!(closure.root(), &root);
    assert_eq!(closure.packages().len(), 4);
}

#[test]
fn unreachable_package_is_rejected() {
    let root = key("application", "application");
    let unused = key("unused", "unused");
    let errors = ResolvedPackageClosure::new(
        root.clone(),
        crate::manifest::BuildDeclarationKind::Package,
        vec![node(&root, 1, vec![]), node(&unused, 2, vec![])],
    )
    .unwrap_err();

    assert!(errors.iter().any(|error| matches!(
        error,
        PackageClosureValidationError::UnreachablePackage { key } if key == &unused
    )));
}

#[test]
fn duplicate_alias_within_one_requester_is_rejected() {
    let root = key("application", "application");
    let first = key("first", "first");
    let second = key("second", "second");
    let errors = ResolvedPackageClosure::new(
        root.clone(),
        crate::manifest::BuildDeclarationKind::Package,
        vec![
            node(
                &root,
                1,
                vec![dependency("codec", &first), dependency("codec", &second)],
            ),
            node(&first, 2, vec![]),
            node(&second, 3, vec![]),
        ],
    )
    .unwrap_err();

    assert!(errors.iter().any(|error| matches!(
        error,
        PackageClosureValidationError::DuplicateAlias { requester, alias }
            if requester == &root && alias.as_str() == "codec"
    )));
}

#[test]
fn dependency_cycle_is_rejected() {
    let root = key("application", "application");
    let library = key("library", "library");
    let errors = ResolvedPackageClosure::new(
        root.clone(),
        crate::manifest::BuildDeclarationKind::Package,
        vec![
            node(&root, 1, vec![dependency("library", &library)]),
            node(&library, 2, vec![dependency("application", &root)]),
        ],
    )
    .unwrap_err();

    assert!(errors.iter().any(|error| matches!(
        error,
        PackageClosureValidationError::DependencyCycle { cycle }
            if cycle.first() == cycle.last() && cycle.len() == 3
    )));
}
