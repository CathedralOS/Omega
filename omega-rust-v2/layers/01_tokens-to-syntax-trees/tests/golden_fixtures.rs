//! Runs the accept and reject fixtures under `tests/fixtures/<feature>/` against their expected dumps.

mod support;

use omega_lexer::lex;
use omega_parser::parse_file;
use std::path::{Path, PathBuf};
use support::{describe_tree_problem, dump_parse_error, dump_tree};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Every accept fixture is a whole file: it must parse with `parse_file`, the strict entry point.
#[test]
fn accept_fixtures_parse_to_their_expected_trees() {
    let report = golden::check_features(&fixtures(), "accept", |_, source_bytes| {
        let tokens = lex(source_bytes).map_err(|lex_error| format!("an accept fixture does not lex: {lex_error}"))?;
        let syntax_tree = parse_file(source_bytes, &tokens)
            .map_err(|parse_error| format!("an accept fixture was rejected:\n{}", dump_parse_error(source_bytes, &parse_error)))?;

        spec_corpus::observe_handoff!(
            "token-coverage-of-all-source-bytes/consumer",
            if let Some(problem) = describe_tree_problem(source_bytes, &tokens, &syntax_tree) {
                return Err(format!("the tree is malformed: {problem}"));
            }
        );

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
    let report = golden::check_features(&fixtures(), "reject", |_, source_bytes| {
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
fn trait_body_separator_is_diagnosed_only_after_a_complete_header() {
    use omega_parser::ParseErrorKind::{SeparatorBeforeBody, UnexpectedToken};

    for header in ["trait T", "trait U = T"] {
        let good = format!("{header} {{}}");
        let bytes = good.as_bytes();
        parse_file(bytes, &lex(bytes).unwrap()).expect(&good);
    }

    for (prefix, token, rest, kind, expected) in [
        ("trait T ", ",", " {}", SeparatorBeforeBody, "`{`"),
        ("trait U = T ", ";", " {}", SeparatorBeforeBody, "`{`"),
        ("trait U = ", ",", " {}", UnexpectedToken, "a type"),
        ("trait T ", ";", "", UnexpectedToken, "`{`"),
    ] {
        let source = format!("{prefix}{token}{rest}");
        let bytes = source.as_bytes();
        let error = parse_file(bytes, &lex(bytes).unwrap()).expect_err(&source);
        assert_eq!((error.get_kind(), error.get_expected()), (kind, expected), "{source}");
        assert_eq!(error.get_span().get_start_offset(), prefix.len(), "{source}");
        assert_eq!(error.get_span().slice(bytes), Some(token.as_bytes()), "{source}");
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

#[test]
fn dangling_selection_path_reports_the_first_fixable_occurrence() {
    use omega_parser::NodeKind;
    use omega_parser::ParseErrorKind::{UnexpectedEnd, UnexpectedToken};

    let header = "trait Trait { machine slot(); machine other(); }\nmachine Ops::target() {}\nImpl satisfies Trait {\n";
    let first = "    Trait::slot = Ops::;\n";
    let first_fixed = "    Trait::slot = Ops::target;\n";
    let later = "    Trait::other = ;\n";
    let later_fixed = "    Trait::other = Ops::target;\n";
    let mut observations = Vec::new();
    for source in [
        format!("{header}{first}{later}}}\n"),
        format!("{header}{first_fixed}{later}}}\n"),
        format!("{header}{first}{later_fixed}}}\n"),
        format!("{header}{}", first.trim_end_matches(";\n")),
    ] {
        let tokens = lex(source.as_bytes()).unwrap();
        let error = parse_file(source.as_bytes(), &tokens).expect_err(&source);
        let span = error.get_span();
        observations.push((error.get_kind(), error.get_expected(), span.get_start_offset()..span.get_end_offset()));
    }

    let source = format!("{header}{first_fixed}{later_fixed}}}\n");
    let tokens = lex(source.as_bytes()).unwrap();
    let tree = parse_file(source.as_bytes(), &tokens).expect("repairing both rows should parse");
    let indexed = tree.index(&source, &tokens).unwrap();
    let rows: Vec<_> = indexed
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::SelectionRow)
        .map(|row| {
            row.children
                .iter()
                .map(|&child| {
                    assert_eq!(indexed.nodes[child].kind, NodeKind::Path);
                    let span = indexed.span(child);
                    &source[span.get_start_offset()..span.get_end_offset()]
                })
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(rows, [["Trait::slot", "Ops::target"], ["Trait::other", "Ops::target"]]);

    spec_corpus::observe_handoff!(
        "syntax-trait-conformance/first-malformed-selection",
        assert_eq!(
            observations,
            [
                (UnexpectedToken, "a name after `::`", 120..121),
                (UnexpectedToken, "a name", 147..148),
                (UnexpectedToken, "a name after `::`", 120..121),
                (UnexpectedEnd, "a name after `::`", 120..120),
            ]
        )
    );
}
