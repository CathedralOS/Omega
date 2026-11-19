//! Parses every `omega` example in the specification.

mod support;

use omega_lexer::lex;
use omega_parser::{FragmentKind, ParseErrorKind, parse_fragment};
use spec_corpus::parser_examples::{EXAMPLE_SHAPES, fragment_kind, parser_source};
use std::path::Path;
use support::dump_parse_error;

/// Statement fragments may mix items and tails. Clauses and single parameters must consume
/// exactly their own grammar, including the optional final semicolon of a clause fragment.
#[test]
fn fragment_entry_points_consume_their_entire_input() {
    for (kind, source) in [
        (FragmentKind::Statements, "use a; b() c()"),
        (FragmentKind::Clauses, "requires a ensures b;"),
        (FragmentKind::GenericParam, "const N: u32"),
    ] {
        let tokens = lex(source.as_bytes()).unwrap();
        let tree = parse_fragment(source.as_bytes(), &tokens, kind).unwrap_or_else(|error| panic!("{source}: {error}"));
        assert_eq!(support::describe_tree_problem(source.as_bytes(), &tokens, &tree), None, "{source}");
    }
    for (kind, source, expected_span) in [
        (FragmentKind::Statements, "use a; )", 7..8),
        (FragmentKind::Clauses, "requires a; extra", 12..17),
        (FragmentKind::GenericParam, "const N: u32, T", 12..13),
    ] {
        let tokens = lex(source.as_bytes()).unwrap();
        let error = parse_fragment(source.as_bytes(), &tokens, kind).expect_err(source);
        assert_eq!(error.get_kind(), ParseErrorKind::UnexpectedToken, "{source}");
        let span = error.get_span();
        assert_eq!(span.get_start_offset()..span.get_end_offset(), expected_span, "{source}");
    }
}

/// Every example in the spec must parse, and its tree must pass the grammar-independent checks.
///
/// A failure here means one of three things: the parser is wrong, GRAMMAR.md is missing a form the
/// spec uses, or the example is not valid V2 source. Each needs a look; none should be silenced by
/// skipping the example.
#[test]
fn every_spec_example_parses() {
    let spec_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../wiki/spec");
    let examples = spec_corpus::extract_omega_examples(&spec_directory).expect("Expected wiki/spec to be readable.");

    assert!(examples.len() >= 100, "found only {} spec examples; is wiki/spec complete?", examples.len());

    for (shaped_label, _) in EXAMPLE_SHAPES {
        assert!(
            examples
                .iter()
                .any(|example| example.get_label() == *shaped_label),
            "EXAMPLE_SHAPES names {shaped_label}, but no example starts there; has the spec moved?"
        );
    }

    let mut problems = Vec::new();

    for example in &examples {
        let example_label = example.get_label();
        // Same length as the original, so byte offsets in an error still point into the spec's text.
        let source_text = parser_source(&example.source_text);
        let source_bytes = source_text.as_bytes();

        let tokens = match lex(source_bytes) {
            Ok(tokens) => tokens,
            Err(lex_error) => {
                problems.push(format!("{example_label}\ndoes not lex: {lex_error}"));
                continue;
            }
        };

        match parse_fragment(source_bytes, &tokens, fragment_kind(&example_label)) {
            Err(parse_error) => {
                let error_line = get_line_text(&source_text, parse_error.get_span().get_start_offset());

                problems.push(format!("{example_label}\n{}  line: {error_line}", dump_parse_error(source_bytes, &parse_error)));
            }
            Ok(syntax_tree) => {
                if let Some(problem) = support::describe_tree_problem(source_bytes, &tokens, &syntax_tree) {
                    problems.push(format!("{example_label}\nthe tree is malformed: {problem}"));
                }
            }
        }
    }

    assert!(
        problems.is_empty(),
        "{} of {} spec examples have a problem:\n\n{}",
        problems.len(),
        examples.len(),
        problems.join("\n\n")
    );
    println!("parsed {} spec examples", examples.len());
}

/// The line of `source_text` that holds `byte_offset`, for showing where an error is.
fn get_line_text(
    source_text: &str,
    byte_offset: usize,
) -> &str {
    let line_start = source_text
        .get(..byte_offset)
        .and_then(|preceding_text| preceding_text.rfind('\n'))
        .map_or(0, |line_feed_offset| line_feed_offset + 1);

    source_text
        .get(line_start..)
        .and_then(|remaining_text| remaining_text.lines().next())
        .unwrap_or_default()
}
