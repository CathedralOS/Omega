mod support;
use omega_lexer::{LineIndex, SourceSpan};
use omega_typer::ValueType;
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
    for (directory, accepts) in [("accept", true), ("reject", false)] {
        let report = golden::check_directory(&root.join(directory), "omg", |bytes| {
            let source = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
            let authored = |s: SourceSpan| std::str::from_utf8(s.slice(source.as_bytes()).unwrap()).unwrap();
            let mut lines = Vec::new();
            match support::check(&[("fixture.omg", source)]) {
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
                            row.conformance.path,
                            row.requirement.path,
                            types(&row.trait_arguments),
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
                    for (id, ty) in stage.constants {
                        lines.push(format!("const | {} | {}", id.path, ty));
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
