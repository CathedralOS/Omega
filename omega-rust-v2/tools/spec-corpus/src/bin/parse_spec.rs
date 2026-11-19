//! Prints the tree the parser builds for the spec's examples, so a person can read what it decided.
//!
//! Every example, source first and tree after:
//!
//!     mbx run --locked -p spec-corpus --features parser --bin parse_spec
//!
//! Only the examples whose label contains some text, where a label is `file:line`:
//!
//!     mbx run --locked -p spec-corpus --features parser --bin parse_spec -- language/patterns.md

use omega_lexer::lex;
use omega_parser::parse_fragment;
use spec_corpus::parser_examples::{fragment_kind, parser_source};
use spec_corpus::tree_dump_formatter::{dump_parse_error, dump_tree};
use std::path::Path;

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

        let fragment_kind = fragment_kind(&example_label);
        let source_text = parser_source(&example.source_text);
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
