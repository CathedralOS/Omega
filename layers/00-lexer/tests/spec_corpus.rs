//! Lexes every `omega` example in the specification.

mod support;

use omega_lexer::{dump_error, lex};
use std::path::Path;

/// Every example in the spec must lex, tile its source, and have self-delimiting tokens.
///
/// A failure here means one of three things: the lexer is wrong, the contract is missing a rule the
/// spec relies on, or the spec example is itself lexically invalid. Each needs a look; none should
/// be silenced by skipping the example.
#[test]
fn every_spec_example_lexes() {
    let spec_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../wiki/spec");
    let examples =
        spec_corpus::extract_omega_examples(&spec_directory).expect("wiki/spec should be readable");
    assert!(
        examples.len() >= 100,
        "found only {} spec examples; is wiki/spec complete?",
        examples.len()
    );

    let mut problems = Vec::new();
    for example in &examples {
        let source_bytes = example.source_text.as_bytes();
        match lex(source_bytes) {
            Err(error) => problems.push(format!(
                "{}\n{}",
                example.get_label(),
                dump_error(source_bytes, &error)
            )),
            Ok(tokens) => {
                if let Some(problem) = support::describe_tiling_problem(source_bytes, &tokens) {
                    problems.push(format!(
                        "{}\ntokens do not tile the source: {problem}",
                        example.get_label()
                    ));
                }
                if let Some(problem) = support::describe_relex_problem(source_bytes, &tokens) {
                    problems.push(format!(
                        "{}\na token is not self-delimiting: {problem}",
                        example.get_label()
                    ));
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
    println!("lexed {} spec examples", examples.len());
}
