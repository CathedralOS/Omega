//! Prints the tree the parser builds for the spec's examples, so a person can read what it decided.
//!
//! Every example, source first and tree after:
//!
//!     cargo run -p omega-parser --example parse_spec
//!
//! Only the examples whose label contains some text, where a label is `file:line`:
//!
//!     cargo run -p omega-parser --example parse_spec -- language/patterns.md

// The dump is the tests' own; see the note at the top of that file for why it is not in the crate.
#[path = "../tests/support/tree_dump_formatter.rs"]
mod tree_dump_formatter;

use omega_lexer::lex;
use omega_parser::{FragmentKind, parse_fragment};
use std::path::Path;
use tree_dump_formatter::{dump_parse_error, dump_tree};

// @Volatile: must list the same examples as `EXAMPLE_SHAPES` in `tests/spec_corpus.rs`. The test is
// the authority; this copy only decides how an example is shown.
const EXAMPLE_SHAPES: &[(&str, FragmentKind)] = &[
    ("language/generics.md:306", FragmentKind::Clauses),
    ("language/termination.md:35", FragmentKind::Clauses),
    ("resources/content_custody.md:129", FragmentKind::Clauses),
    ("language/state_contracts.md:86", FragmentKind::GenericParam),
];

fn main() {
    let label_filter = std::env::args().nth(1).unwrap_or_default();
    let spec_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../wiki/spec");

    let examples = match spec_corpus::extract_omega_examples(&spec_directory) {
        Ok(examples) => examples,
        Err(read_error) => {
            eprintln!("could not read {}: {read_error}", spec_directory.display());
            std::process::exit(1);
        }
    };

    for example in &examples {
        let example_label = example.get_label();

        if !example_label.contains(&label_filter) {
            continue;
        }

        let fragment_kind = EXAMPLE_SHAPES
            .iter()
            .find(|(shaped_label, _)| *shaped_label == example_label)
            .map_or(FragmentKind::Statements, |&(_, fragment_kind)| fragment_kind);
        let source_text = example.source_text.replace("{ ... }", "{     }");
        let source_bytes = source_text.as_bytes();

        println!("===== {example_label} ({fragment_kind:?})");
        println!("{source_text}");

        let tokens = match lex(source_bytes) {
            Ok(tokens) => tokens,
            Err(lex_error) => {
                println!("does not lex: {lex_error}\n");
                continue;
            }
        };

        match parse_fragment(source_bytes, &tokens, fragment_kind) {
            Ok(syntax_tree) => println!("{}", dump_tree(source_bytes, &tokens, &syntax_tree)),
            Err(parse_error) => println!("{}", dump_parse_error(source_bytes, &parse_error)),
        }
    }
}
