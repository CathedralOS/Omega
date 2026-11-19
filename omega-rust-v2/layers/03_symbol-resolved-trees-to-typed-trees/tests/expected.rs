#[path = "support/evaluation.rs"]
mod evaluation;
mod support;
use omega_lexer::{LineIndex, SourceSpan};
use omega_typer::{Site, SpecializedIndexValue, TypingEnvironment, ValueType};
use std::path::Path;

fn types(values: &[ValueType]) -> String {
    values
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}
#[test]
fn typed_fixtures_match_handwritten_expectations() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    for (directory, accepts, evaluation) in [
        ("accept", true, None),
        ("reject", false, None),
        ("prepared/accept", true, Some((7u8, 8u16))),
        ("prepared/reject", false, Some((255, 256))),
    ] {
        let report = golden::check_directory(&root.join(directory), "omg", |bytes| {
            let source = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
            let authored = |s: SourceSpan| std::str::from_utf8(s.slice(source.as_bytes()).unwrap()).unwrap();
            let mut lines = Vec::new();
            let files = [("fixture.omg", source)];
            let checked = if let Some((input, result)) = evaluation {
                let offset = source.find("width(N)").expect("prepared fixture calls width");
                let receipt = SpecializedIndexValue {
                    request: omega_typer::StaticValueRequest {
                        node: evaluation::node(source, offset, "width(N)", omega_parser::NodeKind::Call),
                        expression: Site {
                            package: "game".into(),
                            file: "fixture.omg".into(),
                            span: SourceSpan::new(offset, offset + "width(N)".len()),
                        },
                        carrier: omega_typer::ValueType::Integer { signed: false, width: Some(8) },
                    },
                    arguments: vec![ValueType::StaticInteger {
                        carrier: Box::new(ValueType::Integer { signed: false, width: Some(8) }),
                        value: input.into(),
                    }],
                    value: omega_typer::StaticValue::Integer(result.into()),
                };
                support::check_in(
                    &files,
                    &TypingEnvironment {
                        specialized_indices: &[receipt],
                        ..Default::default()
                    },
                )
            } else {
                support::check(&files)
            };
            match checked {
                Err(e) if accepts => return Err(format!("unexpected refusal at {}: {}", authored(e.span), e.message)),
                Ok(_) if !accepts => return Err("expected a typing refusal".into()),
                Err(e) => {
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
                            "call | {} | {}::{}{} | {}",
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
