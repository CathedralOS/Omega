//! Runs the accept and reject fixtures under `fixtures/` against their expected dumps.

mod support;

use omega_lexer::lex;
use omega_parser::{dump_parse_error, dump_tree, parse_file};
use std::path::{Path, PathBuf};

fn get_fixture_directory(group: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(group)
}

/// Every accept fixture is a whole file: it must parse with `parse_file`, the strict entry point.
#[test]
fn accept_fixtures_parse_to_their_expected_trees() {
    let report = golden::check_directory(&get_fixture_directory("accept"), "omg", |source_bytes| {
        let tokens = lex(source_bytes).map_err(|lex_error| format!("an accept fixture does not lex: {lex_error}"))?;
        let syntax_tree = parse_file(source_bytes, &tokens)
            .map_err(|parse_error| format!("an accept fixture was rejected:\n{}", dump_parse_error(source_bytes, &parse_error)))?;

        if let Some(problem) = support::describe_tree_problem(source_bytes, &tokens, &syntax_tree) {
            return Err(format!("the tree is malformed: {problem}"));
        }

        Ok(dump_tree(source_bytes, &tokens, &syntax_tree))
    })
    .expect("Expected the accept fixture directory to be readable.");

    assert!(report.case_count > 0, "no accept fixtures were found");

    if let Some(failures) = report.describe_failures() {
        panic!("{failures}");
    }
}

/// Every reject fixture lexes, and then fails to parse with exactly the error its dump records.
#[test]
fn reject_fixtures_fail_with_their_expected_error() {
    let report = golden::check_directory(&get_fixture_directory("reject"), "omg", |source_bytes| {
        let tokens = lex(source_bytes).map_err(|lex_error| format!("a reject fixture must lex, so that it is the parser refusing it: {lex_error}"))?;

        match parse_file(source_bytes, &tokens) {
            Ok(syntax_tree) => Err(format!("a reject fixture was accepted:\n{}", dump_tree(source_bytes, &tokens, &syntax_tree))),
            Err(parse_error) => Ok(dump_parse_error(source_bytes, &parse_error)),
        }
    })
    .expect("Expected the reject fixture directory to be readable.");

    assert!(report.case_count > 0, "no reject fixtures were found");

    if let Some(failures) = report.describe_failures() {
        panic!("{failures}");
    }
}
