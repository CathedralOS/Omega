use super::*;

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed)
}

#[test]
fn binary_float_results_do_not_change_format_at_destinations() {
    for (source_format, target_format) in [("f64", "f32"), ("f32", "f64")] {
        let value = format!("1.0{source_format} + 2.0{source_format}");
        for source in [
            format!("machine take(value: {target_format}) {{}} machine run() {{ take({value}); }}"),
            format!(
                "machine take(value: {target_format}) -> {target_format} {{ value }} machine run() {{ let result: {target_format} = take({value}); }}"
            ),
            format!("machine run() {{ let result: {target_format} = {value}; }}"),
            format!(
                "operator Values::take(value: {target_format}) -> {target_format}; machine run() {{ Values::take({value}); }}"
            ),
            format!(
                "operator Values::take(value: {target_format}) -> {target_format}; machine run() -> {target_format} {{ Values::take({value}) }}"
            ),
            format!("machine run() {{ let result: [{target_format}; 1] = [{value}]; }}"),
            format!(
                "data Item {{ value: {target_format}; }} machine run() {{ let result: Item = Item {{ value: {value} }}; }}"
            ),
            format!(
                "data Main {{}} machine Main::take(&self, value: {target_format}) {{}} machine Main::run(&self) {{ self.take({value}); }}"
            ),
            format!(
                "data Main {{ value: {target_format}; }} machine Main::run(&mut self) {{ self.value = {value}; }}"
            ),
            format!("machine run() -> {target_format} {{ {value} }}"),
            format!("machine run() -> {target_format} {{ transition {{ _ -> {value} }} }}"),
            format!(
                "machine run() {{ transition {{ _ -> next({value}) }} state next(value: {target_format}) {{}} }}"
            ),
        ] {
            let diagnostics = check(&source)
                .err()
                .unwrap_or_else(|| panic!("accepted incompatible destination: {source}"));
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("explicit conversion")),
                "{source}\n{diagnostics:#?}"
            );
        }
    }
}

#[test]
fn binary_operands_keep_declared_call_result_formats() {
    for (operand, result) in [("f64", "f32"), ("f32", "f64")] {
        let source = format!(
            "machine value() -> {operand} {{ 1.0{operand} }} machine run() -> {result} {{ value() + value() }}"
        );
        assert!(
            check(&source).is_err(),
            "call operands retain their format: {source}"
        );
    }
}

#[test]
fn boundary_call_results_keep_their_declared_formats() {
    for (operand, result) in [("f64", "f32"), ("f32", "f64")] {
        let declaration = format!("boundary trait Source {{ machine value() -> {operand}; }}");
        let source = format!(
            "{declaration} machine run() -> {result} {{ Source::value() + Source::value() }}"
        );
        let diagnostics = check(&source)
            .err()
            .unwrap_or_else(|| panic!("boundary result format mismatch accepted: {source}"));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("explicit conversion")),
            "{diagnostics:#?}"
        );
        check(&format!(
            "{declaration} machine run() -> {operand} {{ Source::value() + Source::value() }}"
        ))
        .expect("matching boundary result");
    }
}

#[test]
fn assignment_through_mutable_reference_checks_the_referent() {
    for (operand, result) in [("f64", "f32"), ("f32", "f64")] {
        let source = format!(
            "machine write(destination: &mut {result}, left: {operand}, right: {operand}) {{ destination = left + right; }}"
        );
        let diagnostics = check(&source)
            .err()
            .unwrap_or_else(|| panic!("reference assignment format mismatch accepted: {source}"));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("explicit conversion")),
            "{diagnostics:#?}"
        );
        check(&format!("machine write(destination: &mut {operand}, left: {operand}, right: {operand}) {{ destination = left + right; }}")).expect("matching referent format");
    }
}

#[test]
fn explicit_conversion_result_supplies_its_own_format() {
    for (operand, result, namespace) in [("f64", "f32", "F32"), ("f32", "f64", "F64")] {
        let source = format!(
            "operator {namespace}::convert(value: {operand}) -> {result}; machine run() -> {result} {{ {namespace}::convert(1.0{operand} + 2.0{operand}) }}"
        );
        check(&source).expect("explicit selected conversion");
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
            check(&source).is_err(),
            "method argument format mismatch accepted: {source}"
        );
    }
    check(&format!(
        "{declaration} machine run(source: Source) {{ source.take(1.0f32 + 2.0f32); }}"
    ))
    .expect("receiver is not a positional argument");
}

#[test]
fn binary_float_results_keep_matching_and_anonymous_destinations() {
    for format in ["f32", "f64"] {
        for value in [format!("1.0{format} + 2.0{format}"), "1.0 + 2.0".to_owned()] {
            check(&format!(
                "machine take(value: {format}) {{}} machine run() {{ take({value}); }}"
            ))
            .expect("matching argument");
            check(&format!("machine run() -> {format} {{ {value} }}")).expect("matching result");
        }
    }
}

#[test]
fn selected_heterogeneous_operator_result_controls_destination() {
    for (operand, result) in [("f64", "f32"), ("f32", "f64")] {
        let declaration = format!(
            "operator + {operand}::combine(left: {operand}, right: {operand}) -> {result};"
        );
        check(&format!("{declaration} machine run(left: {operand}, right: {operand}) -> {result} {{ left + right }}")).expect("selected result matches");
        let source = format!(
            "{declaration} machine run(left: {operand}, right: {operand}) -> {operand} {{ left + right }}"
        );
        assert!(
            check(&source).is_err(),
            "selected result cannot adopt operand format: {source}"
        );
    }
}

#[test]
fn only_active_domain_operator_result_controls_destination() {
    let declaration =
        "domain f64::Narrowed; operator + f64::Narrowed::combine(left: f64, right: f64) -> f32;";
    check(&format!(
        "{declaration} machine run(left: f64 in Narrowed, right: f64) -> f32 {{ left + right }}"
    ))
    .expect("active domain selected result");
    check(&format!(
        "{declaration} machine run(left: f64, right: f64) -> f64 {{ left + right }}"
    ))
    .expect("inactive domain preserves builtin");
    for source in [
        format!(
            "{declaration} machine run(left: f64 in Narrowed, right: f64) -> f64 {{ left + right }}"
        ),
        format!("{declaration} machine run(left: f64, right: f64) -> f32 {{ left + right }}"),
    ] {
        assert!(
            check(&source).is_err(),
            "destination does not select the domain: {source}"
        );
    }
}
