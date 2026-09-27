use numerics::literals::FloatFormat;
use source::SourceMap;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use validation::{land_float_literal_destinations, validate_program};

fn typed(source: &str) -> TypedTrees {
    let mut sources = SourceMap::default();
    let source_id = sources
        .add("literal_destinations.omg".into(), source.to_owned())
        .source_id;
    let mut program =
        crate::front_end::typed_program_from_source_map(sources, &[(source_id, source)]);
    land_float_literal_destinations(&mut program);
    program
}

fn rejects_float_format(source: &str) {
    let diagnostics = validate_program(&typed(source))
        .expect_err("a landed float cannot implicitly change its format");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("f32")
                && diagnostic.message.contains("f64")
                && diagnostic
                    .message
                    .contains("a landed float retains its format")
        }),
        "missing float format rejection: {diagnostics:#?}\n{source}"
    );
}

#[test]
fn same_format_values_and_explicit_widening_remain_accepted() {
    for source in [
        "machine take(value: f32) {} machine run(value: f32) { take(value); }",
        "machine take(value: f64) {} machine run(value: f32) { take(value as f64); }",
        "machine take(value: f32) {} machine run() { take(1.0f32 + 2.0f32); }",
        "machine take(value: f64) -> f64 { value } machine run(value: f64) -> f64 { take(value) }",
        "data Main { value: f32; } machine take(value: f32) {} machine Main::run(&self) { take(self.value); }",
    ] {
        let result = validate_program(&typed(source));
        assert!(result.is_ok(), "{result:#?}\n{source}");
    }
}

#[test]
fn float_storage_and_returns_cannot_erase_the_source_format() {
    for source in [
        "machine run(value: f64) { let narrow: f32 = value; }",
        "data Main { value: f32; } machine Main::run(&mut self, value: f64) { self.value = value; }",
        "data Item { value: f32; } machine run(value: f64) { let item: Item = Item { value: value }; }",
        "machine run(value: f64) -> f32 { value }",
        "machine run(value: f64) -> f32 { transition { _ -> (value) } }",
        "machine run(value: f64) { let values: [f32; 1] = [value]; }",
        "machine wide() -> f64 { 1.0f64 } machine take(value: f32) {} machine run() { take(wide()); }",
    ] {
        rejects_float_format(source);
    }
}

#[test]
fn anonymous_direct_returns_round_once_at_the_declared_format() {
    let program = typed("machine value() -> f32 { 16777217.0 }");
    validate_program(&program).expect("an anonymous return adopts its declared format");
    let literals = program
        .expression_table
        .expression_entries()
        .filter_map(|(_, expression)| {
            if let ExpressionNode::Float(literal) = expression {
                Some((literal.landing(), literal.landed_f64()))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert!(!literals.is_empty());
    assert!(
        literals
            .iter()
            .all(|literal| *literal == (Some(FloatFormat::F32), 16_777_216.0)),
        "{literals:?}"
    );
}
