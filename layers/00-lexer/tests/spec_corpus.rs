//! Lexes every `omega` example in the specification.

mod support;

use omega_lexer::lex;
use std::path::Path;
use support::dump_error;

/// Every example in the spec must lex, tile its source, and have self-delimiting tokens.
///
/// A failure here means one of three things: the lexer is wrong, the contract is missing a rule the
/// spec relies on, or the spec example is itself lexically invalid. Each needs a look; none should
/// be silenced by skipping the example.
#[test]
fn every_spec_example_lexes() {
    let spec_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../wiki/spec");
    let spec_examples = spec_corpus::extract_omega_examples(&spec_directory).expect("Expected wiki/spec to be readable.");

    assert!(
        spec_examples.len() >= 100,
        "found only {} spec examples; is wiki/spec complete?",
        spec_examples.len()
    );

    let mut example_problems = Vec::new();

    for spec_example in &spec_examples {
        let source_bytes = spec_example.source_text.as_bytes();

        match lex(source_bytes) {
            Err(lex_error) => example_problems.push(format!("{}\n{}", spec_example.get_label(), dump_error(source_bytes, &lex_error))),
            Ok(tokens) => {
                if let Some(tiling_problem) = support::describe_tiling_problem(source_bytes, &tokens) {
                    example_problems.push(format!("{}\ntokens do not tile the source: {tiling_problem}", spec_example.get_label()));
                }

                if let Some(relex_problem) = support::describe_relex_problem(source_bytes, &tokens) {
                    example_problems.push(format!("{}\na token is not self-delimiting: {relex_problem}", spec_example.get_label()));
                }
            }
        }
    }

    assert!(
        example_problems.is_empty(),
        "{} of {} spec examples have a problem:\n\n{}",
        example_problems.len(),
        spec_examples.len(),
        example_problems.join("\n\n")
    );

    println!("lexed {} spec examples", spec_examples.len());
}
