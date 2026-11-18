//! Runs the accept and reject fixtures under `fixtures/` against their expected dumps.

mod support;

use omega_lexer::{dump_error, dump_tokens, lex};
use std::path::{Path, PathBuf};

fn get_fixture_directory(group: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(group)
}

#[test]
fn accept_fixtures_lex_to_their_expected_tokens() {
    let report = golden::check_directory(&get_fixture_directory("accept"), "omg", |source_bytes| {
        let tokens = lex(source_bytes).map_err(|error| {
            format!(
                "an accept fixture was rejected:\n{}",
                dump_error(source_bytes, &error)
            )
        })?;
        if let Some(problem) = support::describe_tiling_problem(source_bytes, &tokens) {
            return Err(format!("tokens do not tile the source: {problem}"));
        }
        if let Some(problem) = support::describe_relex_problem(source_bytes, &tokens) {
            return Err(format!("a token is not self-delimiting: {problem}"));
        }
        Ok(dump_tokens(source_bytes, &tokens))
    })
    .expect("the accept fixture directory should be readable");

    assert!(report.case_count > 0, "no accept fixtures were found");
    if let Some(failures) = report.describe_failures() {
        panic!("{failures}");
    }
}

#[test]
fn reject_fixtures_fail_with_their_expected_error() {
    let report =
        golden::check_directory(
            &get_fixture_directory("reject"),
            "omg",
            |source_bytes| match lex(source_bytes) {
                Ok(tokens) => Err(format!(
                    "a reject fixture was accepted:\n{}",
                    dump_tokens(source_bytes, &tokens)
                )),
                Err(error) => Ok(dump_error(source_bytes, &error)),
            },
        )
        .expect("the reject fixture directory should be readable");

    assert!(report.case_count > 0, "no reject fixtures were found");
    if let Some(failures) = report.describe_failures() {
        panic!("{failures}");
    }
}
