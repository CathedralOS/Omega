use super::PackageFixture;
use crate::declarations::dependencies::read::DependencyProjectionError;

#[test]
fn rejects_wrong_receiver_and_dependency_argument_shapes() {
    let cases = [
        (
            r#"machine build(builder: &mut Build) { other.depend(Source::Path { location: "x" }); }"#,
            "receiver",
        ),
        (
            r#"machine build(builder: &mut Build) { builder.depend("alias", Source::Path { location: "x" }); }"#,
            "arguments",
        ),
        (
            r#"machine build(builder: &mut Build) { builder.depend_as(Source::Path { location: "x" }); }"#,
            "arguments",
        ),
        (
            r#"machine build(builder: &mut Build) { builder.depend_as("alias", Source::Path { location: "x" }, "extra"); }"#,
            "arguments",
        ),
    ];
    for (source, expected) in cases {
        let fixture = PackageFixture::with_source(source);
        let error = fixture.extract().unwrap_err();
        assert!(
            matches!(
                (&error, expected),
                (
                    DependencyProjectionError::WrongDependencyReceiver,
                    "receiver"
                ) | (
                    DependencyProjectionError::WrongDependencyArguments,
                    "arguments"
                )
            ),
            "unexpected error: {error:?}"
        );
    }
}

#[test]
fn rejects_nonliteral_non_utf8_and_invalid_explicit_aliases() {
    let nonliteral = PackageFixture::with_source(
        r#"machine build(builder: &mut Build) { builder.depend_as(alias, Source::Path { location: "x" }); }"#,
    );
    assert_eq!(
        nonliteral.extract().unwrap_err(),
        DependencyProjectionError::AliasNotString
    );

    let non_utf8 = PackageFixture::with_source(
        r#"machine build(builder: &mut Build) { builder.depend_as("\xff", Source::Path { location: "x" }); }"#,
    );
    assert_eq!(
        non_utf8.extract().unwrap_err(),
        DependencyProjectionError::AliasNotUtf8
    );

    for alias in ["BadAlias", "bad-alias", "_bad", "bad__alias", "bad_"] {
        let fixture = PackageFixture::with_source(&format!(
            r#"machine build(builder: &mut Build) {{ builder.depend_as("{alias}", Source::Path {{ location: "x" }}); }}"#,
        ));
        assert_eq!(
            fixture.extract().unwrap_err(),
            DependencyProjectionError::InvalidAlias {
                alias: alias.to_owned()
            }
        );
    }
}

#[test]
fn rejects_nonliteral_wrong_type_missing_and_unknown_source_cases() {
    let cases = [
        ("source", DependencyProjectionError::SourceNotLiteral),
        (
            r#"Other::Path { location: "x" }"#,
            DependencyProjectionError::WrongSourceType,
        ),
        (
            r#"Other::Source::Path { location: "x" }"#,
            DependencyProjectionError::WrongSourceType,
        ),
        (
            r#"Source::Nested::Path { location: "x" }"#,
            DependencyProjectionError::WrongSourceType,
        ),
        (
            r#"Source { location: "x" }"#,
            DependencyProjectionError::MissingSourceCase,
        ),
        (
            r#"Source::Archive { location: "x" }"#,
            DependencyProjectionError::UnsupportedSourceCase {
                case_name: "Archive".to_owned(),
            },
        ),
    ];
    for (source, expected) in cases {
        let fixture = PackageFixture::with_source(&format!(
            "machine build(builder: &mut Build) {{ builder.depend({source}); }}"
        ));
        assert_eq!(fixture.extract().unwrap_err(), expected);
    }
}

#[test]
fn rejects_missing_extra_duplicate_and_nonliteral_source_fields() {
    for source in [
        "Source::Path {}",
        r#"Source::Path { path: "x" }"#,
        r#"Source::Path { location: "x", extra: "y" }"#,
        r#"Source::Path { location: "x", location: "y" }"#,
        r#"Source::Git { repository: "x" }"#,
        r#"Source::Git { repository: "x", revision: "y", revision: "z" }"#,
    ] {
        let fixture = PackageFixture::with_source(&format!(
            "machine build(builder: &mut Build) {{ builder.depend({source}); }}"
        ));
        assert!(matches!(
            fixture.extract(),
            Err(DependencyProjectionError::WrongSourceFields { .. })
        ));
    }

    let nonliteral = PackageFixture::with_source(
        "machine build(builder: &mut Build) { builder.depend(Source::Path { location: path }); }",
    );
    assert!(matches!(
        nonliteral.extract(),
        Err(DependencyProjectionError::SourceFieldNotString { .. })
    ));
    let non_utf8 = PackageFixture::with_source(
        r#"machine build(builder: &mut Build) { builder.depend(Source::Path { location: "\xff" }); }"#,
    );
    assert!(matches!(
        non_utf8.extract(),
        Err(DependencyProjectionError::SourceFieldNotUtf8 { .. })
    ));
}

#[test]
fn rejects_noncanonical_git_package_selections() {
    let cases = [
        ("selection", DependencyProjectionError::SelectionNotLiteral),
        (
            "Other::Root {}",
            DependencyProjectionError::WrongSelectionType,
        ),
        (
            "Other::PackageSelection::Root {}",
            DependencyProjectionError::WrongSelectionType,
        ),
        (
            "PackageSelection {}",
            DependencyProjectionError::MissingSelectionCase,
        ),
        (
            "PackageSelection::All {}",
            DependencyProjectionError::UnsupportedSelectionCase {
                case_name: "All".to_owned(),
            },
        ),
        (
            "PackageSelection::Root { package: \"matrix\" }",
            DependencyProjectionError::WrongSelectionFields {
                case_name: "Root".to_owned(),
            },
        ),
        (
            "PackageSelection::Named {}",
            DependencyProjectionError::WrongSelectionFields {
                case_name: "Named".to_owned(),
            },
        ),
        (
            "PackageSelection::Named { package: \"Bad_Name\" }",
            DependencyProjectionError::InvalidSelectedPackage {
                package: "Bad_Name".to_owned(),
            },
        ),
    ];
    for (selection, expected) in cases {
        let fixture = PackageFixture::with_source(&format!(
            "machine build(builder: &mut Build) {{ builder.depend(Source::Git {{ repository: \"https://example.invalid/repo.git\", revision: \"main\", selection: {selection} }}); }}"
        ));
        assert_eq!(fixture.extract().unwrap_err(), expected);
    }
}
