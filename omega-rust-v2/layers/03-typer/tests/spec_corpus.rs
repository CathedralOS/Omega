use omega_lexer::lex;
use omega_parser::{FragmentKind, parse_fragment};
use omega_resolver::{ParsedSource, StageSource, resolve_stage};
use std::path::Path;

#[test]
fn spec_fragments_have_reviewed_typing_outcomes() {
    let spec = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../wiki/spec");
    let examples = spec_corpus::extract_omega_examples(&spec).unwrap();
    let mut outcomes = String::new();
    assert_eq!(examples.len(), 132);
    for example in examples {
        let label = example.get_label();
        let kind = match label.as_str() {
            "language/generics.md:306" | "language/termination.md:35" | "resources/content_custody.md:129" => FragmentKind::Clauses,
            "language/state_contracts.md:86" => FragmentKind::GenericParam,
            _ => FragmentKind::Statements,
        };
        let text = example.source_text.replace("{ ... }", "{     }");
        let tokens = lex(text.as_bytes()).unwrap();
        let tree = parse_fragment(text.as_bytes(), &tokens, kind).unwrap();
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
            Ok(stage) => match omega_typer::type_stage(&files, &stage) {
                Ok(_) => "accepted".to_owned(),
                Err(e) => {
                    assert_eq!(e.file, "example.omg");
                    let text = std::str::from_utf8(e.span.slice(text.as_bytes()).unwrap()).unwrap();
                    format!("L3 {} | {}", e.message, text.lines().next().unwrap_or(""))
                }
            },
            Err(error) => {
                let row = format!("{label}  {}", error.message);
                assert!(
                    include_str!("../../02-resolver/fixtures/spec-outcomes.txt")
                        .lines()
                        .any(|l| l == row),
                    "{row}"
                );
                continue;
            }
        };
        outcomes.push_str(&format!("{label}  {outcome}\n"));
    }
    assert_eq!(
        outcomes.lines().collect::<Vec<_>>(),
        include_str!("../fixtures/spec-outcomes.txt")
            .lines()
            .collect::<Vec<_>>()
    );
}
