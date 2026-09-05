use numerics::literals::FloatFormat;
use source::SourceMap;
use source_files_to_tokens::Lexer;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;
use validation::{land_float_literal_destinations, validate_program};

fn typed(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize literals");
    let mut sources = SourceMap::default();
    let source_id = sources
        .add("literal_destinations.omg".into(), source.to_owned())
        .source_id;
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens)
        .expect("parse literals");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
        &syntax,
        std::sync::Arc::new(sources),
    )
    .expect("resolve literal destinations");
    let mut program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type literal destinations");
    land_float_literal_destinations(&mut program);
    program
}

fn rejects_suffix(source: &str, suffix: &str, destination: &str) {
    let diagnostics =
        validate_program(&typed(source)).expect_err("a conflicting suffix must reject");
    let expected = format!("is suffixed `{suffix}` but lands in a `{destination}` place");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(&expected)),
        "missing exact suffix rejection {expected}: {diagnostics:#?}\n{source}"
    );
}

#[test]
fn attached_statement_arguments_preserve_their_explicit_format() {
    rejects_suffix(
        "data Main {} machine Main::take(&self, value: f32) {} machine Main::run(&self) { self.take(1.0f64); }",
        "f64",
        "f32",
    );
}

#[test]
fn free_statement_arguments_reject_either_conflicting_format() {
    for (suffix, destination) in [("f64", "f32"), ("f32", "f64")] {
        rejects_suffix(
            &format!(
                "machine take(value: {destination}) {{}} machine run() {{ take(1.0{suffix}); }}"
            ),
            suffix,
            destination,
        );
    }
}

#[test]
fn expression_call_arguments_preserve_their_explicit_format() {
    rejects_suffix(
        "machine identity(value: f32) -> f32 { value } machine run() { let result: f32 = identity(1.0f64); }",
        "f64",
        "f32",
    );
}

#[test]
fn named_transition_arguments_preserve_their_explicit_format() {
    rejects_suffix(
        "machine run() { transition { _ -> next(1.0f64) } state next(value: f32) {} }",
        "f64",
        "f32",
    );
}

#[test]
fn declared_returns_preserve_their_explicit_format() {
    rejects_suffix("machine value() -> f32 { 1.0f64 }", "f64", "f32");
}

#[test]
fn existing_storage_destination_rejections_remain_intact() {
    for source in [
        "machine run() { let value: f32 = 1.0f64; }",
        "data Main { value: f32; } machine Main::run(&mut self) { self.value = 1.0f64; }",
        "data Item { value: f32; } machine run() { let item: Item = Item { value: 1.0f64 }; }",
    ] {
        rejects_suffix(source, "f64", "f32");
    }
}

#[test]
fn integer_suffixes_obey_the_same_argument_destination_rule() {
    for source in [
        "machine take(value: u32) {} machine run() { take(1u64); }",
        "machine identity(value: u32) -> u32 { value } machine run() { let result: u32 = identity(1u64); }",
        "machine run() { transition { _ -> next(1u64) } state next(value: u32) {} }",
    ] {
        rejects_suffix(source, "u64", "u32");
    }
}

#[test]
fn matching_and_anonymous_arguments_remain_accepted() {
    for source in [
        "machine take(value: f32) {} machine run() { take(1.0f32); take(1.0); }",
        "machine identity(value: f32) -> f32 { value } machine run() { let result: f32 = identity(1.0f32); }",
        "machine run() { transition { _ -> next(1.0f32) } state next(value: f32) {} }",
        "machine value() -> f32 { 1.0f32 }",
        "machine take(value: u32) {} machine run() { take(1u32); take(1); }",
    ] {
        let result = validate_program(&typed(source));
        assert!(result.is_ok(), "{result:#?}\n{source}");
    }
}

#[test]
fn explicit_casts_remain_conversion_boundaries() {
    let program = typed("machine take(value: f32) {} machine run() { take(1.0f64 as f32); }");
    validate_program(&program).expect("an explicit exact cast chooses the destination format");
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
fn named_float_arguments_cannot_implicitly_change_format() {
    for (source, destination) in [("f64", "f32"), ("f32", "f64")] {
        rejects_float_format(&format!(
            "machine take(value: {destination}) {{}} machine run(value: {source}) {{ take(value); }}"
        ));
        rejects_float_format(&format!(
            "machine take(value: {destination}) -> {destination} {{ value }} machine run(value: {source}) -> {destination} {{ take(value) }}"
        ));
        rejects_float_format(&format!(
            "machine run(value: {source}) {{ transition {{ _ -> next(value) }} state next(value: {destination}) {{}} }}"
        ));
    }
}

#[test]
fn float_places_retain_their_format_when_passed_as_arguments() {
    for source in [
        "data Main { value: f64; } machine take(value: f32) {} machine Main::run(&self) { take(self.value); }",
        "machine take(value: f32) {} machine run(values: [f64; 2]) { take(values[0]); }",
        "machine take(value: f32) {} machine run() { let value: f64 = 1.0; take(value); }",
    ] {
        rejects_float_format(source);
    }
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
fn float_comparisons_do_not_inherit_the_operand_format() {
    for source in [
        "machine take(value: bool) {} machine run(value: f64) { take(value == value); }",
        "machine run(value: f64) -> bool { value == value }",
        "machine take(value: f64) {} machine run(value: f64) { take(value + 1.0); }",
    ] {
        let result = validate_program(&typed(source));
        assert!(result.is_ok(), "{result:#?}\n{source}");
    }
}

#[test]
fn authored_operator_results_define_the_delivered_format() {
    let source = "operator + Projection::combine(left: f64, right: f64) -> f32; machine take(value: f32) {} machine run(left: f64, right: f64) { take(left + right); }";
    let result = validate_program(&typed(source));
    assert!(result.is_ok(), "{result:#?}\n{source}");
}

#[test]
fn explicit_cast_outputs_retain_their_selected_format() {
    rejects_float_format(
        "machine take(value: f32) {} machine run(value: f32) { take(value as f64); }",
    );
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

#[test]
fn statement_argument_rounding_does_not_take_an_intermediate_binary64_step() {
    let program =
        typed("machine take(value: f32) {} machine run() { take(8388609.499999999999999); }");
    validate_program(&program).expect("an anonymous argument rounds directly into binary32");
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
            .all(|literal| *literal == (Some(FloatFormat::F32), 8_388_609.0)),
        "{literals:?}"
    );
}

#[test]
fn anonymous_statement_arguments_land_at_the_exact_selected_parameters() {
    let program = typed(
        "data Other {} machine Other::take(&self, first: f64, second: f32) {} data Main {} machine Main::take(&self, first: f32, second: f64) {} machine Main::run(&self) { self.take(16777217.0, 16777217.0); }",
    );
    validate_program(&program).expect("anonymous arguments may land at different formats");
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::run")
        .unwrap();
    let mut literals = Vec::new();
    for state in program.machine_states(machine) {
        for statement in program.statement_table.statements(state.statement_nodes) {
            if let StatementNode::Call(call) = statement {
                for argument in program.statement_table.expression_handles(call.arguments) {
                    let ExpressionNode::Float(literal) =
                        program.expression_table.expression(*argument)
                    else {
                        panic!("expected a literal argument");
                    };
                    literals.push((literal.landing(), literal.landed_f64()));
                }
            }
        }
    }
    assert_eq!(
        literals,
        [
            (Some(FloatFormat::F32), 16_777_216.0),
            (Some(FloatFormat::F64), 16_777_217.0),
        ]
    );
}
