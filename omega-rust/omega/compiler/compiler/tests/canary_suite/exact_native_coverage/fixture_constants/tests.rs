use super::super::ExactNativeCanaryCoverageIndex;
use super::*;
use std::path::Path;

const REPOSITORY_MACRO: &str = r#"
    macro_rules! repository_fixture {
        ($short:ident, $relative:ident, $path:literal) => {
            pub(crate) const $short: &str = $path;
            pub(crate) const $relative: &str = concat!("tests/omega/pass/", $path);
        };
    }
"#;

#[test]
fn exact_local_repository_macro_retains_both_named_paths() {
    let leaf = format!(
        "{REPOSITORY_MACRO}\nrepository_fixture!(EXAMPLE, REPOSITORY_EXAMPLE, \"demo/example\");"
    );
    let constants = load(DECLARATION, |_| Ok(leaf.clone())).unwrap();
    assert_eq!(
        constants.get("EXAMPLE").map(String::as_str),
        Some("demo/example")
    );
    assert_eq!(
        constants.get("REPOSITORY_EXAMPLE").map(String::as_str),
        Some("tests/omega/pass/demo/example")
    );
    assert!(
        index(&rooted("fixture_roster::EXAMPLE"), &leaf)
            .unique_rooted_owner("demo/example")
            .is_some()
    );
    assert_eq!(
        pass_canaries(
            "pass_canary(fixture_roster::REPOSITORY_EXAMPLE)",
            &constants
        ),
        ["tests/omega/pass/demo/example"]
    );
}

#[test]
fn absent_altered_and_ambiguous_repository_macros_do_not_resolve() {
    let invocation_before_definition = format!(
        "repository_fixture!(EXAMPLE, REPOSITORY_EXAMPLE, \"demo/example\");\n{REPOSITORY_MACRO}"
    );
    assert_eq!(
        index(
            &rooted("fixture_roster::EXAMPLE"),
            &invocation_before_definition
        )
        .qualifying_test_count(),
        0
    );
    for definition in [
        String::new(),
        REPOSITORY_MACRO.replace("tests/omega/pass/", "tests/omega/fail/"),
        REPOSITORY_MACRO.replace("= $path;", "= \"demo/wrong\";"),
        REPOSITORY_MACRO.replace("$path:literal", "$path:expression"),
        REPOSITORY_MACRO.replace("pub(crate)", "pub"),
        format!("{REPOSITORY_MACRO}{REPOSITORY_MACRO}"),
        format!("/* {REPOSITORY_MACRO} */"),
        format!("const FAKE: &str = r###\"{REPOSITORY_MACRO}\"###;"),
        format!("#[cfg(windows)]\n{REPOSITORY_MACRO}"),
        format!("{REPOSITORY_MACRO}\n#[cfg(windows)]\n{REPOSITORY_MACRO}"),
    ] {
        let leaf = format!(
            "{definition}\nrepository_fixture!(EXAMPLE, REPOSITORY_EXAMPLE, \"demo/example\");"
        );
        assert_eq!(
            index(&rooted("fixture_roster::EXAMPLE"), &leaf).qualifying_test_count(),
            0
        );
    }
}

#[test]
fn repository_macro_invocations_reject_collisions_and_nonliteral_arguments() {
    let invocation = "repository_fixture!(EXAMPLE, REPOSITORY_EXAMPLE, \"demo/example\");";
    for invocations in [
        format!("{invocation}{invocation}"),
        format!("{LEAF}{invocation}"),
        format!("{invocation}{LEAF}"),
        "repository_fixture!(EXAMPLE, EXAMPLE, \"demo/example\");".into(),
        "repository_fixture!(EXAMPLE, REPOSITORY_EXAMPLE, OTHER);".into(),
        "repository_fixture!(EXAMPLE, REPOSITORY_EXAMPLE, \"demo/example\",);".into(),
        "repository_fixture!(EXAMPLE, \"not_an_identifier\", \"demo/example\");".into(),
    ] {
        let leaf = format!("{REPOSITORY_MACRO}\n{invocations}");
        assert!(load(DECLARATION, |_| Ok(leaf.clone())).is_err());
    }
    let leaf = format!(
        "{REPOSITORY_MACRO}\n/* {invocation} */\nconst FAKE: &str = r###\"{invocation}\"###;"
    );
    assert!(load(DECLARATION, |_| Ok(leaf.clone())).unwrap().is_empty());
}

const DECLARATION: &str =
    "#[path = \"../fixture_rosters/example.rs\"]\npub(super) mod fixture_roster;\n";
const LEAF: &str =
    "pub const EXAMPLE: &str = \"demo/example\";\npub const OTHER: &str = \"demo/other\";";

fn rooted(argument: &str) -> String {
    format!(
        "#[test]\nfn native() {{ let canary = pass_canary({argument}); \
        compile_rooted_canary_for_native_host(&canary, build).unwrap(); \
        let output = Command::new(path).output().unwrap(); \
        assert_eq!(output.status.code(), Some(70)); }}"
    )
}

fn index(body: &str, leaf: &str) -> ExactNativeCanaryCoverageIndex {
    let source = format!("{DECLARATION}{body}");
    let constants = load(&source, |path| {
        assert_eq!(path, "../fixture_rosters/example.rs");
        Ok(leaf.to_owned())
    })
    .unwrap();
    let mut index = ExactNativeCanaryCoverageIndex::empty();
    index.index_source(Path::new("example.rs"), &source, &constants);
    index
}

#[test]
fn named_constants_preserve_native_identity_and_status() {
    for argument in [
        "fixture_roster::EXAMPLE",
        "\nfixture_roster::EXAMPLE,\n",
        "fixture_roster :: EXAMPLE,",
        "\n\"demo/example\",\n",
    ] {
        for visibility in ["pub", "pub(crate)", "pub(super)"] {
            let leaf = LEAF.replace("pub const", &format!("{visibility} const"));
            let index = index(&rooted(argument), &leaf);
            let owner = index.unique_rooted_owner("demo/example").unwrap();
            assert_eq!(owner.test_name, "native");
            assert_eq!(owner.expected_status, 70);
        }
    }
}

#[test]
fn missing_foreign_dynamic_and_multiple_references_fail_closed() {
    for argument in [
        "fixture_roster::MISSING",
        "another_roster::EXAMPLE",
        "EXAMPLE",
        "fixture_roster::PASS_CANARIES[0]",
    ] {
        assert_eq!(index(&rooted(argument), LEAF).qualifying_test_count(), 0);
    }
    for other in [
        "fixture_roster::MISSING",
        "fixture_roster::OTHER",
        "\"demo/other\"",
    ] {
        let body = rooted("fixture_roster::EXAMPLE").replace(
            "let canary =",
            &format!("let other = pass_canary({other}); let canary ="),
        );
        assert_eq!(index(&body, LEAF).qualifying_test_count(), 0);
    }
    let body = rooted("fixture_roster::EXAMPLE");
    let repeated = format!("{body}\n{}", body.replace("fn native", "fn second"));
    let index = index(&repeated, LEAF);
    assert_eq!(index.rooted_owner_count("demo/example"), 2);
    assert!(index.unique_rooted_owner("demo/example").is_none());
}

#[test]
fn comments_and_raw_strings_cannot_declare_or_select_a_fixture() {
    let fake = format!("// {DECLARATION}\nlet text = r###\"{DECLARATION}\"###;");
    assert!(
        load(&fake, |_| panic!("masked declaration cannot load a leaf"))
            .unwrap()
            .is_empty()
    );
    let leaf = format!(
        "// pub const FAKE: &str = \"demo/fake\";\n\
        const TEXT: &str = r###\"pub const RAW: &str = \"demo/raw\";\"###;\n{LEAF}"
    );
    for name in ["FAKE", "RAW"] {
        assert_eq!(
            index(&rooted(&format!("fixture_roster::{name}")), &leaf).qualifying_test_count(),
            0
        );
    }
    let body = rooted("fixture_roster::EXAMPLE").replace(
        "let canary =",
        r####"
        // pass_canary(fixture_roster::OTHER);
        /* pass_canary("demo/comment"); */
        let raw = r###"pass_canary(fixture_roster::OTHER); } #[test] fn invented() {}"###;
        let quoted = "pass_canary(fixture_roster::OTHER)";
        let canary ="####,
    );
    assert!(
        index(&body, &leaf)
            .unique_rooted_owner("demo/example")
            .is_some()
    );
    let only_raw = rooted("fixture_roster::EXAMPLE").replace(
        "pass_canary(fixture_roster::EXAMPLE)",
        "r#\"pass_canary(fixture_roster::EXAMPLE)\"#",
    );
    assert_eq!(index(&only_raw, LEAF).qualifying_test_count(), 0);
}

#[test]
fn only_the_exact_unconditional_owner_leaf_is_loaded() {
    for source in [
        "#[path = \"../fixture_rosters/example.rs\"] mod other;",
        "#[cfg(windows)]\n#[path = \"../fixture_rosters/example.rs\"] mod fixture_roster;",
        "fn helper() { #[path = \"../fixture_rosters/example.rs\"] mod fixture_roster; }",
    ] {
        assert!(
            load(source, |_| panic!("not an unconditional owner declaration"))
                .unwrap()
                .is_empty()
        );
    }
    assert!(load(&format!("{DECLARATION}{DECLARATION}"), |_| Ok(LEAF.into())).is_err());
    assert!(
        load(
            &DECLARATION.replace("../fixture_rosters/example.rs", "../other/example.rs"),
            |_| Ok(LEAF.into())
        )
        .is_err()
    );
    assert!(load(DECLARATION, |_| Err("missing leaf".into())).is_err());
    assert!(load(DECLARATION, |_| Ok(format!("{LEAF}\n{LEAF}"))).is_err());
    let spaced = "pub const EXAMPLE: &str = \"demo /example\";";
    assert_eq!(
        index(&rooted("fixture_roster::EXAMPLE"), spaced).qualifying_test_count(),
        0
    );
}

#[test]
fn named_paths_do_not_relax_cfg_ignore_or_status_guards() {
    let body = rooted("fixture_roster::EXAMPLE");
    for changed in [
        body.replace("#[test]", "#[cfg(windows)]\n#[test]"),
        body.replace("#[test]", "#[test]\n#[ignore]"),
        body.replace("Some(70)", "expected"),
        body.replace(".output()", ".status()"),
    ] {
        assert_eq!(index(&changed, LEAF).qualifying_test_count(), 0);
    }
}

#[test]
fn named_paths_preserve_exact_target_and_entry_guards() {
    let body = r#"
        #[test]
        fn cross() {
            let canary = pass_canary(fixture_roster::EXAMPLE);
            compile(CanaryCompileSpec {
                root_path: canary.join("main.omg"),
                target_name: Some("linux_x86_64".into()),
                product: CanaryCompileProduct::NativeArtifactAndPublish,
            }).unwrap();
        }
    "#;
    assert!(
        index(body, LEAF)
            .unique_cross_target_owner("demo/example", "linux_x86_64")
            .is_some()
    );
    for changed in [
        body.replace("canary.join", "other.join"),
        body.replace("\"linux_x86_64\".into()", "target.into()"),
        body.replace("NativeArtifactAndPublish", "Check"),
        body.replace(".unwrap()", ""),
        body.replace("fixture_roster::EXAMPLE", "fixture_roster::MISSING"),
    ] {
        assert_eq!(index(&changed, LEAF).qualifying_target_compile_count(), 0);
    }
    let rooted = body.replace("compile(CanaryCompileSpec {\n                root_path: canary.join(\"main.omg\"),\n                target_name: Some(\"linux_x86_64\".into()),\n                product: CanaryCompileProduct::NativeArtifactAndPublish,\n            })", "compile_rooted_canary_for_target(&canary, build, \"linux_x86_64\")");
    assert!(
        index(&rooted, LEAF)
            .unique_rooted_target_owner("demo/example", "linux_x86_64")
            .is_some()
    );
}
