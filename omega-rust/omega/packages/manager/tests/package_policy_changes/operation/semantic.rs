use super::*;

#[path = "../../fixture_rosters/semantic_failures.rs"]
mod fixture_roster;

// Existing package_reconstruction_question fixture: ordinary exit checking
// preserves this assumption, but polynomial entailment cannot represent min.
const OPEN_CONTRACT: &str = "pub machine unchecked_claim(left: u64, right: u64)\nrequires min(left, right) >= 1\nensures min(left, right) >= 1\n{}\n";

#[test]
fn unresolved_contracts_reject_for_the_root_and_transitive_packages() {
    for transitive in [false, true] {
        let tree = Tree::new();
        if transitive {
            source(
                &tree,
                PURE,
                " builder.depend(Source::Path { location: \"../middle\" });\n",
            );
            package(
                &tree.path("sources/middle"),
                "middle",
                " builder.depend(Source::Path { location: \"../leaf\" });\n",
            );
            package(&tree.path("sources/leaf"), "unresolved-leaf", "");
            fs::write(tree.path("sources/leaf/main.omg"), OPEN_CONTRACT).unwrap();
        } else {
            source(&tree, OPEN_CONTRACT, "");
        }
        let closure = resolve(&tree, "open-contract");
        let owner = if transitive {
            closure
                .custodies()
                .iter()
                .find(|custody| custody.key().name().as_str() == "unresolved-leaf")
                .unwrap()
                .key()
                .clone()
        } else {
            closure.graph().root().clone()
        };
        let error = review_package_change(closure, TARGET, None, &tree.path("build")).unwrap_err();
        assert!(
            matches!(error, PackageChangeError::UndischargedContract { package, count } if *package == owner && count == 1)
        );
    }
}

#[test]
fn invalid_proof_and_service_reach_remain_compiler_failures() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../tests/omega/fail");
    for case in fixture_roster::FILE_EXPECTATION_FAIL_CANARIES {
        let fixture = corpus.join(case);
        let main = fs::read_to_string(fixture.join("main.omg")).unwrap();
        let expected = fs::read_to_string(fixture.join("expected.txt")).unwrap();
        for transitive in [false, true] {
            let tree = Tree::new();
            if transitive {
                super::transitive::source_chain(&tree, "invalid-leaf", &main);
            } else {
                source(&tree, &main, "");
            }
            let build = fs::read(tree.path("sources/root/build.omg")).unwrap();
            let closure = resolve(&tree, "invalid");
            let owner = if transitive {
                assert_eq!(closure.graph().packages().len(), 3);
                closure
                    .custodies()
                    .iter()
                    .find(|custody| custody.key().name().as_str() == "invalid-leaf")
                    .unwrap()
                    .key()
                    .clone()
            } else {
                closure.graph().root().clone()
            };
            let error =
                review_package_change(closure, TARGET, None, &tree.path("build")).unwrap_err();
            let PackageChangeError::Compilation(CompileResolvedPackageReviewsError::Compilation {
                package,
                diagnostics,
            }) = error
            else {
                panic!("expected compiler failure for {case}, transitive={transitive}: {error:?}");
            };
            assert_eq!(package, owner, "{case}, transitive={transitive}");
            let rendered = diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                rendered.contains(expected.trim()),
                "{case}, transitive={transitive}: {rendered}"
            );
            assert!(!tree.path("sources/root/omega.lock").exists());
            assert_eq!(
                fs::read(tree.path("sources/root/build.omg")).unwrap(),
                build
            );
        }
    }
}

#[test]
fn scoped_build_generated_sources_reach_the_proposed_policy() {
    let tree = Tree::new();
    source(
        &tree,
        PURE,
        r#"
    builder.log.write_line("generated package data");
    let generated: BuildPath = builder.output.resolve("generated.omg");
    let descriptor: i32 = builder.output.create(generated, 438);
    let count: i64 = builder.output.write(descriptor, "pub data Generated { value: u64; }\n");
    let closed: i32 = builder.output.close(descriptor);
    builder.output.include_source(generated);
"#,
    );
    let checked = review(&tree, "generated", None);
    let proposed = propose(&checked);
    let root = checked.source_closure().graph().root();
    let rows = checked
        .changes()
        .packages()
        .iter()
        .find(|package| package.key() == root)
        .unwrap()
        .rows();
    assert!(rows.iter().any(|row| {
        row.kind() == PackagePolicyRowKind::PublicData
            && row
                .candidate()
                .unwrap()
                .canonical_text()
                .contains("Generated")
    }));
    let root_review = checked.reviews().review(root).unwrap();
    let observation = root_review.build_observation_summary().unwrap();
    let expected_log = b"generated package data\n";
    assert_eq!(observation.build_log(), expected_log);
    // Generated-output replay charges the log again. Successful review has
    // reconciled both executions with its sponsor while retaining one log.
    let usage = root_review.build_evaluation_usage().unwrap();
    assert_eq!(usage.build_log_bytes, expected_log.len() as u64);
    assert_eq!(usage.replay_build_log_bytes, expected_log.len() as u64);
    assert!(
        root_review.policy().dangerous_capabilities().is_empty(),
        "compiler-owned logging must not introduce runtime Console authority"
    );
    assert!(
        !checked
            .source_closure()
            .source_root(root)
            .unwrap()
            .join("generated.omg")
            .exists()
    );
    assert_eq!(
        fs::read_dir(tree.path("generated-build")).unwrap().count(),
        0
    );
    assert_round_trip(&checked, proposed);
    assert!(!tree.path("sources/root/omega.lock").exists());
}
