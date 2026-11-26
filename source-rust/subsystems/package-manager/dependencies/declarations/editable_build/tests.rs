//! Only the selected free Build entry and its borrowed receiver identify an edit.
use super::{BuildEntrySyntaxProjection, project_build_entry_syntax};
use crate::declarations::dependencies::read::DependencyProjectionError;
use build_declarations::{ProjectName, WorkspaceMemberPath};

fn project(source: &str) -> Result<BuildEntrySyntaxProjection, DependencyProjectionError> {
    let parsed = source_syntax::parse_text(source).expect("valid editor test fixture");
    project_build_entry_syntax(&parsed.syntax)
}

#[test]
fn the_build_entry_has_exactly_one_mutable_build_parameter() {
    for source in [
        "machine build() {}",
        "machine build(builder: @mutable Build, filesystem: @mutable Filesystem) {}",
        "machine build(builder: Build) {}",
        "machine build(builder: @Build) {}",
        "machine build(builder: @write Build) {}",
        "machine build(builder: @mutable Builder) {}",
        "machine build(filesystem: @mutable Filesystem, builder: @mutable Build) {}",
    ] {
        assert_eq!(
            project(source),
            Err(DependencyProjectionError::InvalidBuildParameter)
        );
    }
}

#[test]
fn rejects_spoofed_toolchain_vocabulary_and_nonordinary_builds() {
    for source in [
        r#"data Build {} machine build(builder: @mutable Build) { Build::package(builder, "spoof"); }"#,
        r#"machine Build::dependency(name: @[u8]) {} machine build(builder: @mutable Build) { Build::package(builder, "spoof"); }"#,
    ] {
        assert!(matches!(
            project(source),
            Err(DependencyProjectionError::AuthoredToolchainVocabulary { .. })
        ));
    }
    assert!(matches!(
        project(
            r#"machine Owner::build(builder: @mutable Build) { Build::package(builder, "scoped"); }"#
        ),
        Err(DependencyProjectionError::ScopedBuildMachine { .. })
    ));
    assert_eq!(
        project("boundary machine build(builder: @mutable Build);"),
        Err(DependencyProjectionError::InvalidBuildMachine)
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
