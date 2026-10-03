//! Tests for projecting build declarations from source and syntax.

use crate::{
    ApplicationDeclaration, BuildDeclaration, BuildDeclarationError, BuildDeclarationKind,
    PackageDeclaration, ProjectName, WorkspaceDeclaration, WorkspaceMemberPath,
    project_build_declaration_from_source, project_build_declaration_syntax,
};
use source_files_to_tokens::Lexer;
use tokens_to_syntax_trees::parse_syntax_trees;

fn project(source: &str) -> Result<BuildDeclaration, BuildDeclarationError> {
    project_build_declaration_from_source(source)
}

#[test]
fn source_and_syntax_apis_project_the_same_authoritative_role() {
    let source = r#"
        machine build(builder: &mut Build) {
            Build::application(builder, "omega_compiler");
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("lex fixture");
    let trees = parse_syntax_trees(&tokens).expect("parse fixture");
    let syntax = project_build_declaration_syntax(&trees).expect("project syntax");

    assert_eq!(
        syntax.declaration(),
        &project_build_declaration_from_source(source).expect("project source")
    );
    assert!(matches!(
        syntax.declaration(),
        BuildDeclaration::Application(application)
            if application.name.as_str() == "omega_compiler"
    ));
    assert_eq!(
        trees
            .items
            .state_parameter(syntax.builder_parameter())
            .name
            .as_str(),
        "builder"
    );
    assert_eq!(
        trees
            .items
            .statements(trees.items.state(syntax.build_entry()).statements)
            .len(),
        1
    );
}

#[test]
fn projects_package_application_and_workspace_declarations() {
    assert!(matches!(
        project(r#"machine build(builder: &mut Build) { Build::package(builder, "exact_math"); }"#),
        Ok(BuildDeclaration::Package(PackageDeclaration { name }))
            if name.as_str() == "exact_math"
    ));
    assert!(matches!(
        project(r#"machine build(builder: &mut Build) { Build::application(builder, "omega"); }"#),
        Ok(BuildDeclaration::Application(ApplicationDeclaration { name, .. }))
            if name.as_str() == "omega"
    ));
    assert_eq!(
        project(
            r#"machine build(builder: &mut Build) { Build::member(builder, "omega/std"); Build::member(builder, "source/compiler"); }"#,
        ),
        Ok(BuildDeclaration::Workspace(WorkspaceDeclaration {
            members: vec![
                WorkspaceMemberPath::parse("omega/std").unwrap(),
                WorkspaceMemberPath::parse("source/compiler").unwrap(),
            ],
        }))
    );
}

#[test]
fn the_build_entry_has_exactly_one_canonical_builder_parameter() {
    for source in [
        "machine build() {}",
        "machine build(builder: &mut Build, filesystem: &mut Filesystem) {}",
        "machine build(builder: Build) {}",
        "machine build(builder: &Build) {}",
        "machine build(builder: &write Build) {}",
        "machine build(builder: &mut Builder) {}",
        "machine build(build: &mut Build) {}",
        "machine build(filesystem: &mut Filesystem, builder: &mut Build) {}",
    ] {
        assert_eq!(
            project(source),
            Err(BuildDeclarationError::InvalidBuildParameter)
        );
    }
}

#[test]
fn rejects_mixed_duplicate_and_hidden_role_syntax() {
    assert_eq!(
        project(
            r#"machine build(builder: &mut Build) { Build::package(builder, "one"); Build::application(builder, "two"); }"#,
        ),
        Err(BuildDeclarationError::MixedBuildDeclarations)
    );
    assert_eq!(
        project(
            r#"machine build(builder: &mut Build) { Build::application(builder, "one"); Build::application(builder, "two"); }"#,
        ),
        Err(BuildDeclarationError::DuplicateApplicationDeclarations { count: 2 })
    );
    assert_eq!(
        project(
            r#"machine helper(builder: &mut Build) { Build::member(builder, "hidden"); } machine build(builder: &mut Build) {}"#,
        ),
        Err(BuildDeclarationError::UnsupportedMemberShape)
    );
}

#[test]
fn rejects_wrong_role_receivers_arguments_and_literals() {
    for (source, expected) in [
        (
            r#"machine build(builder: &mut Build) { other.package("wrong"); }"#,
            BuildDeclarationError::WrongPackageReceiver,
        ),
        (
            r#"machine build(builder: &mut Build) { Build::application(builder); }"#,
            BuildDeclarationError::WrongApplicationArguments,
        ),
        (
            r#"machine build(builder: &mut Build) { Build::member(builder, "one", "two"); }"#,
            BuildDeclarationError::WrongMemberArguments,
        ),
        (
            r#"machine build(builder: &mut Build) { Build::package(builder, package_name); }"#,
            BuildDeclarationError::NameNotStringLiteral,
        ),
        (
            r#"machine build(builder: &mut Build) { Build::member(builder, member_path); }"#,
            BuildDeclarationError::MemberPathNotStringLiteral,
        ),
    ] {
        assert_eq!(project(source), Err(expected), "source: {source}");
    }
    assert_eq!(
        project(r#"machine build(builder: &mut Build) { Build::package(builder, "\x80name"); }"#),
        Err(BuildDeclarationError::NameNotUtf8)
    );
    assert_eq!(
        project(r#"machine build(builder: &mut Build) { Build::member(builder, "\x80path"); }"#),
        Err(BuildDeclarationError::MemberPathNotUtf8)
    );
}

#[test]
fn rejects_duplicate_package_and_workspace_rows() {
    assert_eq!(
        project(
            r#"machine build(builder: &mut Build) { Build::package(builder, "one"); Build::package(builder, "two"); }"#,
        ),
        Err(BuildDeclarationError::DuplicatePackageDeclarations { count: 2 })
    );
    assert_eq!(
        project(
            r#"machine build(builder: &mut Build) { Build::member(builder, "same/path"); Build::member(builder, "same/path"); }"#,
        ),
        Err(BuildDeclarationError::DuplicateWorkspaceMember {
            path: "same/path".to_owned()
        })
    );
}

#[test]
fn rejects_role_calls_outside_the_direct_root_entry_shape() {
    for source in [
        r#"machine helper(builder: &mut Build) { Build::package(builder, "hidden"); } machine build(builder: &mut Build) {}"#,
        r#"machine build(builder: &mut Build) { state later(builder: &mut Build) { Build::package(builder, "nested"); } }"#,
        r#"machine build(builder: &mut Build) { consume(Build::package(builder, "expression")); }"#,
    ] {
        assert_eq!(
            project(source),
            Err(BuildDeclarationError::UnsupportedPackageShape),
            "source: {source}"
        );
    }
}

#[test]
fn source_api_reports_lex_and_parse_failures() {
    assert!(matches!(
        project("machine build(builder: &mut Build) { ` }"),
        Err(BuildDeclarationError::Lex { .. })
    ));
    assert!(matches!(
        project("machine build(builder: &mut Build) {"),
        Err(BuildDeclarationError::Parse { .. })
    ));
}

#[test]
fn rejects_spoofed_toolchain_vocabulary_and_nonordinary_builds() {
    for source in [
        r#"data Build {} machine build(builder: &mut Build) { Build::package(builder, "spoof"); }"#,
        r#"machine Build::package(name: &[u8]) {} machine build(builder: &mut Build) { Build::package(builder, "spoof"); }"#,
    ] {
        assert!(matches!(
            project(source),
            Err(BuildDeclarationError::AuthoredToolchainVocabulary { .. })
        ));
    }
    assert!(matches!(
        project(
            r#"machine Owner::build(builder: &mut Build) { Build::package(builder, "scoped"); }"#
        ),
        Err(BuildDeclarationError::ScopedBuildMachine { .. })
    ));
    assert_eq!(
        project("boundary machine build(builder: &mut Build);"),
        Err(BuildDeclarationError::InvalidBuildMachine)
    );
}

#[test]
fn validated_names_and_member_paths_match_the_declared_canonical_forms() {
    for name in ["arithmetic_kernels", "sha256", "codec_2"] {
        assert_eq!(ProjectName::parse(name).unwrap().as_str(), name);
    }
    for name in [
        "Arithmetic-Kernels",
        "arithmetic-kernels",
        "arithmetic__kernels",
        "_arithmetic",
        "arithmetic_",
        "123-tools",
    ] {
        assert!(ProjectName::parse(name).is_err(), "accepted {name:?}");
    }
    for path in ["", "/absolute", "packages/../escape", "packages//double"] {
        assert!(
            WorkspaceMemberPath::parse(path).is_err(),
            "accepted {path:?}"
        );
    }
}

#[test]
fn artifact_only_is_a_direct_application_modifier_not_a_role() {
    assert_eq!(
        project(
            r#"machine build(builder: &mut Build) { Build::application(builder, "publisher"); Build::artifact_only(builder); }"#,
        ),
        Ok(BuildDeclaration::Application(ApplicationDeclaration {
            name: ProjectName::parse("publisher").unwrap(),
            artifact_only: true,
        }))
    );
    // Modifier order inside the root entry does not matter; the projection
    // still yields the same single application declaration.
    assert_eq!(
        project(
            r#"machine build(builder: &mut Build) { Build::artifact_only(builder); Build::application(builder, "publisher"); }"#,
        ),
        Ok(BuildDeclaration::Application(ApplicationDeclaration {
            name: ProjectName::parse("publisher").unwrap(),
            artifact_only: true,
        }))
    );
    // An ordinary application declaration leaves the flag clear.
    assert_eq!(
        project(
            r#"machine build(builder: &mut Build) { Build::application(builder, "publisher"); }"#,
        ),
        Ok(BuildDeclaration::Application(ApplicationDeclaration {
            name: ProjectName::parse("publisher").unwrap(),
            artifact_only: false,
        }))
    );
}

#[test]
fn artifact_only_rejects_non_application_roles_and_repetition() {
    assert_eq!(
        project(
            r#"machine build(builder: &mut Build) { Build::package(builder, "lib"); Build::artifact_only(builder); }"#,
        ),
        Err(BuildDeclarationError::ArtifactOnlyRequiresApplication {
            found: BuildDeclarationKind::Package
        })
    );
    assert_eq!(
        project(
            r#"machine build(builder: &mut Build) { Build::member(builder, "one"); Build::artifact_only(builder); }"#,
        ),
        Err(BuildDeclarationError::ArtifactOnlyRequiresApplication {
            found: BuildDeclarationKind::Workspace
        })
    );
    assert_eq!(
        project(
            r#"machine build(builder: &mut Build) { Build::artifact_only(builder); Build::artifact_only(builder); Build::application(builder, "app"); }"#,
        ),
        Err(BuildDeclarationError::DuplicateArtifactOnlyDeclarations { count: 2 })
    );
    // The modifier alone never supplies the required role declaration.
    assert_eq!(
        project(r#"machine build(builder: &mut Build) { Build::artifact_only(builder); }"#),
        Err(BuildDeclarationError::MissingBuildDeclaration)
    );
}

#[test]
fn artifact_only_rejects_wrong_receiver_arguments_and_indirect_shapes() {
    for (source, expected) in [
        (
            r#"machine build(builder: &mut Build) { Build::application(builder, "app"); other.artifact_only(); }"#,
            BuildDeclarationError::WrongArtifactOnlyReceiver,
        ),
        (
            r#"machine build(builder: &mut Build) { Build::application(builder, "app"); Build::artifact_only(builder, "app"); }"#,
            BuildDeclarationError::WrongArtifactOnlyArguments,
        ),
        (
            r#"machine helper(builder: &mut Build) { Build::artifact_only(builder); } machine build(builder: &mut Build) { Build::application(builder, "app"); }"#,
            BuildDeclarationError::UnsupportedArtifactOnlyShape,
        ),
        (
            r#"machine build(builder: &mut Build) { Build::application(builder, "app"); consume(Build::artifact_only(builder)); }"#,
            BuildDeclarationError::UnsupportedArtifactOnlyShape,
        ),
        (
            r#"machine build(builder: &mut Build) { Build::application(builder, "app"); state later(builder: &mut Build) { Build::artifact_only(builder); } }"#,
            BuildDeclarationError::UnsupportedArtifactOnlyShape,
        ),
    ] {
        assert_eq!(project(source), Err(expected), "source: {source}");
    }
    assert!(matches!(
        project(
            r#"machine Build::artifact_only() {} machine build(builder: &mut Build) { Build::application(builder, "app"); Build::artifact_only(builder); }"#,
        ),
        Err(BuildDeclarationError::AuthoredToolchainVocabulary { .. })
    ));
}
