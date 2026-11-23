use omega_lexer::lex;
use omega_parser::parse_fragment;
use omega_resolver::{ParsedSource, StageSource, resolve_stage};
use spec_corpus::parser_examples::{fragment_kind, parser_source};

#[test]
fn spec_fragments_have_reviewed_resolution_outcomes() {
    let examples = spec_corpus::published_examples().unwrap();
    let mut outcomes = String::new();
    for example in examples {
        let label = example.get_label();
        let text = parser_source(&example.source_text);
        let tokens = lex(text.as_bytes()).unwrap();
        let tree = match parse_fragment(text.as_bytes(), &tokens, fragment_kind(&label)) {
            Ok(tree) => tree,
            Err(error) => {
                let reason = error.get_kind().get_message(error.get_expected());
                outcomes.push_str(&format!("{label}  L1 {reason}\n"));
                continue;
            }
        };
        let input = StageSource {
            package: "example",
            source: ParsedSource {
                logical_path: "example.omg",
                text: &text,
                tokens: &tokens,
                tree: &tree,
            },
        };
        let files = [input];
        let outcome = match resolve_stage(&files, &[]) {
            Ok(stage) => {
                support::check_origins(&stage, &files);
                "accepted".to_owned()
            }
            Err(error) => {
                assert_eq!(error.file, "example.omg");
                assert!(error.span.get_end_offset() <= text.len());
                error.message
            }
        };
        outcomes.push_str(&format!("{label}  {outcome}\n"));
    }
    assert_eq!(outcomes, include_str!("spec-outcomes.txt"));
}
mod support;
