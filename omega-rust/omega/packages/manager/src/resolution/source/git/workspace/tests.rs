use super::*;
use crate::declarations::{BuildDeclarationKind, DependencyProjectionError};

fn package_name(value: &str) -> PackageName {
    PackageName::parse(value).expect("valid package name")
}

fn member_path(value: &str) -> WorkspaceMemberPath {
    WorkspaceMemberPath::parse(value).expect("valid member path")
}

fn workspace_source(members: &[&str]) -> Vec<u8> {
    let declarations = members
        .iter()
        .map(|member| format!(r#"builder.member("{member}");"#))
        .collect::<Vec<_>>()
        .join(" ");
    format!("machine build(builder: &mut Build) {{ {declarations} }}").into_bytes()
}

fn package_source(name: &str) -> Vec<u8> {
    format!(r#"machine build(builder: &mut Build) {{ builder.package("{name}"); }}"#).into_bytes()
}

#[test]
fn selects_once_and_retains_ordered_replayable_commitments() {
    let root = workspace_source(&["packages/codec", "packages/math"]);
    let codec_path = member_path("packages/codec");
    let math_path = member_path("packages/math");
    let codec = package_source("codec");
    let math = br#"
        machine build(builder: &mut Build) {
            builder.package("exact-math");
            builder.depend(Source::Path { location: "../codec" });
        }
    "#;
    let supplied_out_of_order = [
        GitWorkspaceMemberBuild::new(&math_path, math),
        GitWorkspaceMemberBuild::new(&codec_path, &codec),
    ];

    let discovery = discover_git_workspace(&root).expect("discover workspace declarations");
    assert_eq!(
        discovery
            .member_paths()
            .iter()
            .map(WorkspaceMemberPath::as_str)
            .collect::<Vec<_>>(),
        ["packages/codec", "packages/math"]
    );
    assert!(
        discovery
            .workspace_declaration()
            .commitment()
            .matches(&root)
    );

    let plan =
        plan_git_workspace_selection(&package_name("exact-math"), &root, &supplied_out_of_order)
            .expect("plan selection");

    assert_eq!(plan.selected_member_path().as_str(), "packages/math");
    assert_eq!(plan.selected_member().package_name().as_str(), "exact-math");
    assert_eq!(plan.workspace_declaration().repository_path(), "build.omg");
    assert_eq!(plan.workspace_declaration().byte_count(), root.len());
    assert!(plan.workspace_declaration().commitment().matches(&root));
    assert_eq!(
        plan.members()
            .iter()
            .map(|member| member.member_path().as_str())
            .collect::<Vec<_>>(),
        ["packages/codec", "packages/math"]
    );
    assert_eq!(
        plan.selected_member().declaration().repository_path(),
        "packages/math/build.omg"
    );
    assert!(
        plan.selected_member()
            .declaration()
            .commitment()
            .matches(math)
    );
    assert_eq!(
        plan.selected_member()
            .declaration()
            .commitment()
            .to_hex()
            .len(),
        64
    );
    plan.replay(&root, &supplied_out_of_order)
        .expect("replay exact evidence");
    assert_eq!(
        plan.for_declared_member(&codec_path)
            .expect("retarget declared member")
            .selected_member_path(),
        &codec_path
    );

    let changed_math = package_source("exact-math");
    let changed = [
        GitWorkspaceMemberBuild::new(&math_path, &changed_math),
        GitWorkspaceMemberBuild::new(&codec_path, &codec),
    ];
    assert_eq!(
        plan.replay(&root, &changed),
        Err(GitWorkspaceSelectionError::DeclarationEvidenceChanged)
    );
}

#[test]
fn rejects_missing_extra_and_duplicate_member_inputs() {
    let root = workspace_source(&["a", "b"]);
    let a_path = member_path("a");
    let b_path = member_path("b");
    let extra_path = member_path("extra");
    let a = package_source("a");
    let b = package_source("b");

    assert_eq!(
        plan_git_workspace_selection(
            &package_name("a"),
            &root,
            &[GitWorkspaceMemberBuild::new(&a_path, &a)]
        ),
        Err(GitWorkspaceSelectionError::MissingMemberBuild {
            member_path: b_path.clone()
        })
    );
    assert_eq!(
        plan_git_workspace_selection(
            &package_name("a"),
            &root,
            &[
                GitWorkspaceMemberBuild::new(&a_path, &a),
                GitWorkspaceMemberBuild::new(&b_path, &b),
                GitWorkspaceMemberBuild::new(&extra_path, &b),
            ]
        ),
        Err(GitWorkspaceSelectionError::ExtraMemberBuild {
            member_path: extra_path
        })
    );
    assert_eq!(
        plan_git_workspace_selection(
            &package_name("a"),
            &root,
            &[
                GitWorkspaceMemberBuild::new(&a_path, &a),
                GitWorkspaceMemberBuild::new(&a_path, &a),
                GitWorkspaceMemberBuild::new(&b_path, &b),
            ]
        ),
        Err(GitWorkspaceSelectionError::DuplicateMemberBuild {
            member_path: a_path
        })
    );
}

#[test]
fn rejects_missing_and_duplicate_package_names() {
    let root = workspace_source(&["a", "b"]);
    let a_path = member_path("a");
    let b_path = member_path("b");
    let duplicate_a = package_source("same-name");
    let duplicate_b = package_source("same-name");
    let duplicate_members = [
        GitWorkspaceMemberBuild::new(&a_path, &duplicate_a),
        GitWorkspaceMemberBuild::new(&b_path, &duplicate_b),
    ];

    assert_eq!(
        plan_git_workspace_selection(&package_name("absent"), &root, &duplicate_members),
        Err(GitWorkspaceSelectionError::PackageMissing {
            package_name: package_name("absent")
        })
    );
    assert_eq!(
        plan_git_workspace_selection(&package_name("same-name"), &root, &duplicate_members),
        Err(GitWorkspaceSelectionError::PackageDuplicate {
            package_name: package_name("same-name"),
            member_paths: vec![a_path, b_path]
        })
    );
}

#[test]
fn rejects_non_utf8_and_malformed_declarations_at_their_exact_paths() {
    assert_eq!(
        plan_git_workspace_selection(&package_name("a"), &[0xff], &[]),
        Err(GitWorkspaceSelectionError::NonUtf8Declaration {
            repository_path: "build.omg".to_owned()
        })
    );

    let root = workspace_source(&["members/a"]);
    let path = member_path("members/a");
    assert_eq!(
        plan_git_workspace_selection(
            &package_name("a"),
            &root,
            &[GitWorkspaceMemberBuild::new(&path, &[0xff])]
        ),
        Err(GitWorkspaceSelectionError::NonUtf8Declaration {
            repository_path: "members/a/build.omg".to_owned()
        })
    );

    let malformed = b"machine build(builder: &mut Build) {";
    assert!(matches!(
        plan_git_workspace_selection(
            &package_name("a"),
            &root,
            &[GitWorkspaceMemberBuild::new(&path, malformed)]
        ),
        Err(GitWorkspaceSelectionError::MalformedDeclaration {
            repository_path,
            ..
        }) if repository_path == "members/a/build.omg"
    ));
}

#[test]
fn rejects_root_and_member_role_confusion() {
    let root_package = package_source("root");
    assert_eq!(
        plan_git_workspace_selection(&package_name("root"), &root_package, &[]),
        Err(GitWorkspaceSelectionError::WrongRole {
            repository_path: "build.omg".to_owned(),
            expected: BuildDeclarationKind::Workspace,
            found: BuildDeclarationKind::Package,
        })
    );

    let root = workspace_source(&["nested"]);
    let nested_path = member_path("nested");
    let nested_workspace = workspace_source(&["child"]);
    assert_eq!(
        plan_git_workspace_selection(
            &package_name("nested"),
            &root,
            &[GitWorkspaceMemberBuild::new(
                &nested_path,
                &nested_workspace
            )]
        ),
        Err(GitWorkspaceSelectionError::WrongRole {
            repository_path: "nested/build.omg".to_owned(),
            expected: BuildDeclarationKind::Package,
            found: BuildDeclarationKind::Workspace,
        })
    );
}

#[test]
fn rejects_package_builds_outside_static_dependency_projection_policy() {
    let root = workspace_source(&["packages/a"]);
    let path = member_path("packages/a");
    let hidden_dependency = br#"
        machine helper(builder: &mut Build) {
            builder.depend(Source::Path { location: "hidden" });
        }
        machine build(builder: &mut Build) {
            builder.package("a");
            helper(builder);
        }
    "#;

    assert_eq!(
        plan_git_workspace_selection(
            &package_name("a"),
            &root,
            &[GitWorkspaceMemberBuild::new(&path, hidden_dependency)]
        ),
        Err(GitWorkspaceSelectionError::StaticDependencyProjection {
            member_path: path,
            error: DependencyProjectionError::UnsupportedDependencyShape,
        })
    );
}

#[test]
fn rejects_declarations_over_the_compiler_owned_byte_ceiling() {
    let oversized = vec![b' '; MAX_BUILD_DECLARATION_BYTES + 1];
    assert_eq!(
        plan_git_workspace_selection(&package_name("a"), &oversized, &[]),
        Err(GitWorkspaceSelectionError::ResourceLimit {
            limit: GitWorkspaceSelectionLimit::DeclarationBytes,
            maximum: MAX_BUILD_DECLARATION_BYTES,
            observed: MAX_BUILD_DECLARATION_BYTES + 1,
        })
    );
}
