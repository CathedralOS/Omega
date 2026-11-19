//! Runs the accept and reject fixtures under `fixtures/` against their expected dumps.

mod support;

use omega_lexer::lex;
use std::path::{Path, PathBuf};
use support::{dump_error, dump_tokens};

fn get_fixture_directory(fixture_group: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(fixture_group)
}

#[test]
fn accept_fixtures_lex_to_their_expected_tokens() {
    let golden_report = golden::check_directory(&get_fixture_directory("accept"), "omg", |source_bytes| {
        let tokens = lex(source_bytes).map_err(|lex_error| format!("an accept fixture was rejected:\n{}", dump_error(source_bytes, &lex_error)))?;

        if let Some(tiling_problem) = support::describe_tiling_problem(source_bytes, &tokens) {
            return Err(format!("tokens do not tile the source: {tiling_problem}"));
        }

        if let Some(relex_problem) = support::describe_relex_problem(source_bytes, &tokens) {
            return Err(format!("a token is not self-delimiting: {relex_problem}"));
        }

        Ok(dump_tokens(source_bytes, &tokens))
    })
    .expect("Expected the accept fixture directory to be readable.");

    assert!(golden_report.case_count > 0, "no accept fixtures were found");

    if let Some(failure_description) = golden_report.describe_failures() {
        panic!("{failure_description}");
    }
}

#[test]
fn reject_fixtures_fail_with_their_expected_error() {
    let golden_report = golden::check_directory(&get_fixture_directory("reject"), "omg", |source_bytes| match lex(source_bytes) {
        Ok(tokens) => Err(format!("a reject fixture was accepted:\n{}", dump_tokens(source_bytes, &tokens))),
        Err(lex_error) => Ok(dump_error(source_bytes, &lex_error)),
    })
    .expect("Expected the reject fixture directory to be readable.");

    assert!(golden_report.case_count > 0, "no reject fixtures were found");

    if let Some(failure_description) = golden_report.describe_failures() {
        panic!("{failure_description}");
    }
}
