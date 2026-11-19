use omega_parser::{parse_file, parse_fragment};
use omega_preparation::{PreparedValue, prepare_source_package};
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::resolve_source_package;
use spec_corpus::parser_examples::{fragment_kind, parser_source};
use std::{collections::BTreeMap, path::Path};

#[test]
fn every_spec_fragment_has_a_preparation_outcome() {
    let spec = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../wiki/spec");
    let examples = spec_corpus::extract_omega_examples(&spec).unwrap();
    let previous: BTreeMap<_, _> = include_str!("../../02_syntax-trees-to-symbol-resolved-trees/fixtures/spec-outcomes.txt")
        .lines()
        .chain(include_str!("../../03_symbol-resolved-trees-to-typed-trees/fixtures/spec-outcomes.txt").lines())
        .map(|line| line.split_once("  ").unwrap())
        .collect();
    assert_eq!(examples.len(), previous.len());
    let mut differences = Vec::new();
    for example in examples {
        let label = example.get_label();
        let text = parser_source(&example.source_text);
        let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
        let tree = parse_fragment(text.as_bytes(), &tokens, fragment_kind(&label)).unwrap();
        let files = [StageSource {
            package: "example",
            source: ParsedSource {
                logical_path: "example.omg",
                text: &text,
                tokens: &tokens,
                tree: &tree,
            },
        }];
        let outcome = match resolve_source_package(&files) {
            Err(error) => error.message,
            Ok(package) => match prepare_source_package(&package) {
                Ok(prepared) => {
                    for c in &prepared.typed().constants {
                        assert!(prepared.constant(&c.declaration).is_some(), "{label}: missing {}", c.declaration.path);
                    }
                    "accepted".to_owned()
                }
                Err(error) => {
                    assert_eq!(error.file, "example.omg", "{label}");
                    let occurrence = std::str::from_utf8(error.span.slice(text.as_bytes()).unwrap()).unwrap();
                    format!("{} | {}", error.message, occurrence.lines().next().unwrap_or(""))
                }
            },
        };
        let expected = previous[label.as_str()];
        if outcome != expected.strip_prefix("L3 ").unwrap_or(expected) {
            differences.push(format!("{label}  {outcome}"));
        }
    }
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}
#[test]
fn spec_constants_freeze_by_declaration_including_type_scoped_names() {
    let mut examples = Vec::new();
    spec_corpus::extract_examples_from_markdown("constants", include_str!("../../../wiki/spec/language/constants.md"), &mut examples);
    let declarations = &examples[0].source_text;
    // The published example uses the EFI type without declaring it.
    let source = format!("data EfiStatus [copy] {{code:u64;}}\n{declarations}");
    let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
    let tree = parse_file(source.as_bytes(), &tokens).unwrap();
    let package = resolve_source_package(&[StageSource {
        package: "example",
        source: ParsedSource {
            logical_path: "example.omg",
            text: &source,
            tokens: &tokens,
            tree: &tree,
        },
    }])
    .unwrap();
    let prepared = prepare_source_package(&package).unwrap();
    let declaration = |name| package.declarations.iter().find(|id| id.path == name).unwrap();
    assert_eq!(prepared.constant(declaration("PAGE_SIZE")), Some(&PreparedValue::Integer(4096.into())));
    assert_eq!(
        prepared.constant(declaration("EfiStatus::SUCCESS")),
        Some(&PreparedValue::Record {
            owner: declaration("EfiStatus").clone(),
            fields: [PreparedValue::Integer(0.into())].into()
        })
    );
}
