//! Parses every `omega` example in the specification.

mod support;

use omega_lexer::lex;
use omega_parser::{FragmentKind, parse_fragment};
use std::path::Path;
use support::dump_parse_error;

/*
    The spec's examples are V2's nearest thing to a conformance corpus, and
    most of them are not whole files: they are a few statements, or a
    declaration with a statement after it to show a call. So every example is
    parsed as `FragmentKind::Statements`, which takes declarations and
    statements in any order, unless it is listed here as one of the two
    narrower shapes.

    Examples are named by file and by the line of their opening fence. If the
    spec is edited above one of these, its line moves and this table must
    follow; the test says so rather than quietly parsing the example as the
    wrong shape, because a clause list parsed as statements fails loudly.
*/
const EXAMPLE_SHAPES: &[(&str, FragmentKind)] = &[
    ("language/generics.md:306", FragmentKind::Clauses),
    ("language/termination.md:35", FragmentKind::Clauses),
    ("resources/content_custody.md:129", FragmentKind::Clauses),
    ("language/state_contracts.md:86", FragmentKind::GenericParam),
];

/// The spec writes a body it does not want to show as `{ ... }`. That is prose, not source.
const ELIDED_BODY: &str = "{ ... }";
const EMPTY_BODY: &str = "{     }";

fn get_fragment_kind(example_label: &str) -> FragmentKind {
    EXAMPLE_SHAPES
        .iter()
        .find(|(shaped_label, _)| *shaped_label == example_label)
        .map_or(FragmentKind::Statements, |&(_, fragment_kind)| fragment_kind)
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
        let source_text = example.source_text.replace(ELIDED_BODY, EMPTY_BODY);
        let source_bytes = source_text.as_bytes();

        let tokens = match lex(source_bytes) {
            Ok(tokens) => tokens,
            Err(lex_error) => {
                problems.push(format!("{example_label}\ndoes not lex: {lex_error}"));
                continue;
            }
        };

        match parse_fragment(source_bytes, &tokens, get_fragment_kind(&example_label)) {
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
