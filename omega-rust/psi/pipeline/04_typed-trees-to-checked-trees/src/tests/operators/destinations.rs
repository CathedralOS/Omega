use crate::tests::front_end::checked_program_result;

#[test]
fn boundary_call_results_keep_their_declared_formats() {
    for (operand, result) in [("f64", "f32"), ("f32", "f64")] {
        let declaration = format!("boundary trait Source {{ machine value() -> {operand}; }}");
        let source = format!(
            "{declaration} machine run() -> {result} reaches Source {{ Source::value() + Source::value() }}"
        );
        let diagnostics = checked_program_result(&source)
            .err()
            .unwrap_or_else(|| panic!("boundary result format mismatch accepted: {source}"));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("explicit conversion")),
            "{diagnostics:#?}"
        );
        checked_program_result(&format!(
            "{declaration} machine run() -> {operand} reaches Source {{ Source::value() + Source::value() }}"
        ))
        .expect("matching boundary result");
    }
}

#[test]
fn named_operator_method_arguments_exclude_the_implicit_receiver() {
    let declaration = "data Source {} operator Source::take(source: Source, value: f32) -> f32;";
    for body in [
        "source.take(1.0f64 + 2.0f64);",
        "let result: f32 = source.take(1.0f64 + 2.0f64);",
    ] {
        let source = format!("{declaration} machine run(source: Source) {{ {body} }}");
        assert!(
            checked_program_result(&source).is_err(),
            "method argument format mismatch accepted: {source}"
        );
    }
    checked_program_result(&format!(
        "{declaration} machine run(source: Source) {{ source.take(1.0f32 + 2.0f32); }}"
    ))
    .expect("receiver is not a positional argument");
}
