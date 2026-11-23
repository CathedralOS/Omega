#[path = "support/evaluation.rs"]
mod evaluation;
mod support;
use omega_lexer::{LineIndex, SourceSpan};
use omega_typer::{Site, SpecializedIndexValue, TypingEnvironment, ValueType};
use std::path::Path;

/// Extension of the file that holds the static answers a fixture's typing pass receives.
const ANSWER_EXTENSION: &str = "answer";

fn integer_carrier(spelling: &str) -> Result<ValueType, String> {
    let (signed, width) = match spelling.split_at_checked(1) {
        Some(("u", width)) => (false, width),
        Some(("i", width)) => (true, width),
        _ => return Err(format!("expected an integer carrier such as u8, found `{spelling}`")),
    };
    let width = width.parse().map_err(|_| format!("expected an integer carrier such as u8, found `{spelling}`"))?;
    Ok(ValueType::Integer { signed, width: Some(width) })
}

/// Typing a generic owner such as `Buffer<width(N)>` needs the value of `width(N)` for each
/// static argument, which preparation normally evaluates. A fixture states those values in a
/// sibling `.answer` file so its program and the answer it assumes are read together.
///
/// Each line is `<call text> | <carrier> | <argument> -> <value>`. The answer applies to the
/// first place the call text appears in the fixture.
fn supplied_answers(fixture: &Path, source: &str) -> Result<Vec<SpecializedIndexValue>, String> {
    let path = fixture.with_extension(ANSWER_EXTENSION);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("could not read {}: {error}", path.display())),
    };
    let mut answers = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let malformed = || format!("{}: expected `<call text> | <carrier> | <argument> -> <value>`, found `{line}`", path.display());
        let [call, carrier, integers] = line.split(" | ").collect::<Vec<_>>()[..] else {
            return Err(malformed());
        };
        let (argument, value) = integers.split_once(" -> ").ok_or_else(malformed)?;
        let argument: i128 = argument.trim().parse().map_err(|_| malformed())?;
        let value: i128 = value.trim().parse().map_err(|_| malformed())?;
        let carrier = integer_carrier(carrier).map_err(|reason| format!("{}: {reason}", path.display()))?;
        let offset = source
            .find(call)
            .ok_or_else(|| format!("{}: the fixture does not contain `{call}`", path.display()))?;
        answers.push(SpecializedIndexValue {
            request: omega_typer::StaticValueRequest {
                node: evaluation::node(source, offset, call, omega_parser::NodeKind::Call),
                expression: Site {
                    package: "game".into(),
                    file: "fixture.omg".into(),
                    span: SourceSpan::new(offset, offset + call.len()),
                },
                carrier: carrier.clone(),
            },
            arguments: vec![ValueType::StaticInteger {
                carrier: Box::new(carrier),
                value: argument.into(),
            }],
            value: omega_typer::StaticValue::Integer(value.into()),
        });
    }
    Ok(answers)
}

fn types(values: &[ValueType]) -> String {
    values
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}
#[test]
fn typed_fixtures_match_handwritten_expectations() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for (directory, accepts, supplies_answers) in [
        ("accept", true, false),
        ("reject", false, false),
        ("prepared/accept", true, true),
        ("prepared/reject", false, true),
    ] {
        let report = golden::check_features(&root, directory, |_fixture, bytes| {
            let source = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
            let authored = |s: SourceSpan| std::str::from_utf8(s.slice(source.as_bytes()).unwrap()).unwrap();
            let mut lines = Vec::new();
            let files = [("fixture.omg", source)];
            let answers = supplied_answers(_fixture, source)?;
            if supplies_answers && answers.is_empty() {
                return Err(format!("a fixture under {directory}/ needs a sibling .{ANSWER_EXTENSION} file with at least one line"));
            }
            let checked = if answers.is_empty() {
                support::check(&files)
            } else {
                support::check_in(
                    &files,
                    &TypingEnvironment {
                        specialized_indices: &answers,
                        ..Default::default()
                    },
                )
            };
            match checked {
                Err(e) if accepts => return Err(format!("unexpected refusal at {}: {}", authored(e.span), e.message)),
                Ok(_) if !accepts => return Err("expected a typing refusal".into()),
                Err(e) => {
                    #[cfg(feature = "diagnostic-origins")]
                    support::report_return(
                        &e,
                        &_fixture
                            .strip_prefix(&root)
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .replace('\\', "/"),
                        None,
                    );
                    let position = LineIndex::new(bytes).get_position(e.span.get_start_offset());
                    lines.push(format!(
                        "refusal | {}:{} | {} | {}",
                        position.get_line(),
                        position.get_column(),
                        authored(e.span),
                        e.message
                    ));
                }
                Ok(stage) => {
                    for op in stage.operations {
                        lines.push(format!(
                            "operator | {} | {} | {} | {}",
                            authored(op.site.span),
                            op.token,
                            types(&op.operands),
                            op.result
                        ));
                    }
                    for call in stage.calls {
                        let args = if call.type_arguments.is_empty() {
                            String::new()
                        } else {
                            format!("<{}>", types(&call.type_arguments))
                        };
                        lines.push(format!(
                            "call | {} | {}\\{}{} | {}",
                            authored(call.site.span),
                            call.machine.package,
                            call.machine.path,
                            args,
                            call.result
                        ));
                    }
                    for row in stage.conformance_rows {
                        lines.push(format!(
                            "row | {} | {}<{}> | {}",
                            row.conformance
                                .as_ref()
                                .map_or_else(|| authored(row.site.span).to_owned(), |c| c.path.clone()),
                            row.requirement.declaration.path,
                            types(&row.requirement.trait_arguments),
                            row.implementation.path
                        ));
                    }
                    for access in stage.collection_accesses {
                        lines.push(format!(
                            "collection | {} | {} | {}",
                            authored(access.site.span),
                            access.machine.path,
                            access.result
                        ));
                    }
                    for constant in stage.constants {
                        lines.push(format!("const | {} | {}", constant.declaration.path, constant.initializer.carrier));
                    }
                    for route in stage.crash_routes {
                        if let Some(guard) = route.guard {
                            lines.push(format!("route | {:?} | {}", route.cause, authored(guard.span)));
                        }
                    }
                    for v in stage.landings {
                        lines.push(format!("landing | {} | {}", authored(v.site.span), v.ty));
                    }
                }
            }
            Ok(format!("{}\n", lines.join("\n")))
        })
        .unwrap();
        assert!(report.case_count > 0);
        if let Some(difference) = report.describe_failures() {
            panic!("{difference}");
        }
    }
}
