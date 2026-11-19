//! Runs the accept and reject fixtures under `fixtures/` against their expected dumps.

mod support;

use omega_lexer::lex;
use omega_parser::parse_file;
use std::path::{Path, PathBuf};
use support::{describe_tree_problem, dump_parse_error, dump_tree};

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

        if let Some(problem) = describe_tree_problem(source_bytes, &tokens, &syntax_tree) {
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
#[test]
fn malformed_selection_rows_refuse_the_first_missing_part() {
    for (row, unexpected) in [
        ("Trait:: = target;", "="),
        ("Trait::slot target;", "target"),
        ("Trait::slot = ;", ";"),
        ("Trait::slot = target", "}"),
    ] {
        let source = format!("Impl satisfies Trait {{ {row} }}");
        let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
        let error = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap_err();
        assert_eq!(error.get_span().slice(source.as_bytes()), Some(unexpected.as_bytes()), "{row}");
    }
}

#[test]
fn bracketed_generic_arguments_have_the_same_shape_in_types_and_expressions() {
    // One bare name inside brackets denotes a slice type. A comma, a call,
    // or parentheses around that same name selects an array of values.
    for (argument, outline) in [
        ("[a]", "SliceType\n  '['\n  PathType 'a'\n  ']'"),
        ("[u8; 4]", "ArrayType\n  '['\n  PathType 'u8'\n  ';'\n  Literal '4'\n  ']'"),
        ("[a, b]", "ArrayLiteral\n  '['\n  PathExpression 'a'\n  ','\n  PathExpression 'b'\n  ']'"),
        (
            "[f(x)]",
            "ArrayLiteral\n  '['\n  Call\n    PathExpression 'f'\n    Args\n      '('\n      PathExpression 'x'\n      ')'\n  ']'",
        ),
        ("[(a)]", "ArrayLiteral\n  '['\n  Paren\n    '('\n    PathExpression 'a'\n    ')'\n  ']'"),
    ] {
        let subtree = outline
            .lines()
            .map(|line| format!("        {line}\n"))
            .collect::<String>();
        for (source, argument_head) in [
            (format!("const v: T<{argument}> = x;"), "TypeArgs\n        '<'"),
            (format!("const v = f::<{argument}>;"), "ExpressionArgs\n        '::'\n        '<'"),
        ] {
            let tokens = lex(source.as_bytes()).unwrap();
            let tree = parse_file(source.as_bytes(), &tokens).expect(&source);
            let dump = dump_tree(source.as_bytes(), &tokens, &tree);
            let expected = format!("      {argument_head}\n{subtree}        '>'\n");
            assert!(dump.contains(&expected), "{source}\nexpected:\n{expected}\nactual:\n{dump}");
        }
    }
}
