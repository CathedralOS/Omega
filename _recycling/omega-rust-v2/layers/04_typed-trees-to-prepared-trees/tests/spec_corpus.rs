use omega_parser::{parse_file, parse_fragment};
use omega_preparation::{PreparedValue, prepare_source_package};
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::resolve_source_package;
use spec_corpus::parser_examples::{fragment_kind, parser_source};
use std::collections::BTreeMap;

/// A retype that consumed L4 answers. L3 owns the refusal and the recorded
/// outcome, but the published stop records that L4 was reached.
const RESUMED_TYPING: &str = "L3-resumed";

struct Refusal {
    /// Stage token for reports: L0..L4 or `L3-resumed`.
    layer: &'static str,
    package: String,
    file: String,
    span: omega_lexer::SourceSpan,
    reason: String,
}

fn inspect_fragment(
    label: &str,
    text: &str,
) -> Result<(), Refusal> {
    let local = |layer, span, reason| Refusal {
        layer,
        package: "example".into(),
        file: "example.omg".into(),
        span,
        reason,
    };
    let tokens = omega_lexer::lex(text.as_bytes()).map_err(|e| local("L0", e.get_span(), e.get_kind().get_message().into()))?;
    let tree =
        parse_fragment(text.as_bytes(), &tokens, fragment_kind(label)).map_err(|e| local("L1", e.get_span(), e.get_kind().get_message(e.get_expected())))?;
    let files = [StageSource {
        package: "example",
        source: ParsedSource {
            logical_path: "example.omg",
            text,
            tokens: &tokens,
            tree: &tree,
        },
    }];
    let package = resolve_source_package(&files).map_err(|e| Refusal {
        layer: "L2",
        package: e.package,
        file: e.file,
        span: e.span,
        reason: e.message,
    })?;
    let prepared = omega_preparation::prepare_source_package_diagnosed(&package).map_err(|failure| {
        use omega_preparation::PreparationFailure::*;
        let (layer, e) = match failure {
            Typing(e) => ("L3", e),
            ResumedTyping(e) => (RESUMED_TYPING, e),
            Preparation(e) | MissingTarget(e) => ("L4", e.diagnostic),
        };
        Refusal {
            layer,
            package: e.package,
            file: e.file,
            span: e.span,
            reason: e.message,
        }
    })?;
    for c in &prepared.typed().constants {
        assert!(prepared.constant(&c.declaration).is_some(), "{label}: missing {}", c.declaration.path);
    }
    Ok(())
}

#[test]
fn every_spec_fragment_has_a_preparation_outcome() {
    let examples = spec_corpus::published_examples().unwrap();
    let resolution: BTreeMap<_, _> = include_str!("../../02_syntax-trees-to-symbol-resolved-trees/tests/spec-outcomes.txt")
        .lines()
        .map(|line| line.split_once("  ").unwrap())
        .collect();
    let typing: BTreeMap<_, _> = include_str!("../../03_symbol-resolved-trees-to-typed-trees/tests/spec-outcomes.txt")
        .lines()
        .map(|line| line.split_once("  ").unwrap())
        .collect();
    let reporting = std::env::var_os("OMEGA_LAYER_REPORT").is_some();
    if reporting {
        println!("OMEGA_FRONTEND_COUNT\t{}", examples.len());
    }
    let mut differences = Vec::new();
    if examples.len() != resolution.len() {
        differences.push("resolution inventory differs from published examples".into());
    }
    for example in examples {
        let label = example.get_label();
        let text = parser_source(&example.source_text);
        let expected = match resolution.get(label.as_str()) {
            Some(&"accepted") => typing
                .get(label.as_str())
                .copied()
                .unwrap_or("<missing typing expectation>")
                .to_owned(),
            Some(&reason) if reason.starts_with("L1 ") => reason.to_owned(),
            Some(reason) => format!("L2 {reason}"),
            None => "<missing resolution expectation>".into(),
        };
        let observation = std::panic::catch_unwind(|| inspect_fragment(&label, &text));
        let actual = match observation {
            Ok(Ok(())) => {
                if reporting {
                    println!("OMEGA_FRONTEND_PREPARED\t{label}");
                }
                "accepted".into()
            }
            Ok(Err(error)) => {
                let local = error.package == "example" && error.file == "example.omg";
                let source = if local {
                    text.get(error.span.get_start_offset()..error.span.get_end_offset())
                } else {
                    None
                };
                if local && source.is_none() {
                    differences.push(format!("{label}: diagnostic span {:?} does not select UTF-8 source text", error.span));
                }
                if reporting {
                    let (line, column) = if source.is_some() {
                        let pos = omega_lexer::LineIndex::new(text.as_bytes()).get_position(error.span.get_start_offset());
                        (pos.get_line().to_string(), pos.get_column().to_string())
                    } else {
                        (String::new(), String::new())
                    };
                    println!(
                        "OMEGA_FRONTEND_STOP\t{label}\t{}\t{}\t{}\t{line}\t{column}\t{}\t{}\t{}\t{}",
                        error.layer,
                        spec_corpus::diagnostic_field(&error.package),
                        spec_corpus::diagnostic_field(&error.file),
                        error.span.get_start_offset(),
                        error.span.get_end_offset(),
                        source.map(spec_corpus::diagnostic_field).unwrap_or_default(),
                        spec_corpus::diagnostic_field(&error.reason)
                    );
                }
                // The snapshot records the owning layer; resumed typing is still an L3 refusal.
                let owner = if error.layer == RESUMED_TYPING { "L3" } else { error.layer };
                let mut outcome = format!("{owner} {}", error.reason);
                if owner == "L3" {
                    outcome.push_str(" | ");
                    outcome.push_str(source.and_then(|s| s.lines().next()).unwrap_or(""));
                }
                outcome
            }
            Err(error) => {
                let message = error
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| error.downcast_ref::<&str>().copied())
                    .unwrap_or("unexpected panic");
                differences.push(format!("{label}: {message}"));
                continue;
            }
        };
        if actual != expected {
            differences.push(format!("{label}: expected {expected}; observed {actual}"));
        }
    }
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}
#[test]
fn spec_constants_freeze_by_declaration_including_type_scoped_names() {
    let examples = spec_corpus::published_chapter("language/constants.md").unwrap();
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
