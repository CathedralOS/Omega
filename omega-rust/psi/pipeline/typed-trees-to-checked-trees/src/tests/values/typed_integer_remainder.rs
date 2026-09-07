use super::*;

const ANONYMOUS_REMAINDERS: [(&str, &str); 3] = [("7 % 2", "1"), ("8 % 2", "0"), ("-3 % 2", "-1")];

fn check(source: &str) -> Result<checked_trees::CheckedTrees, String> {
    let tokens = Lexer::new(source)
        .tokenize()
        .map_err(|error| format!("{error:?}"))?;
    let syntax = parse_syntax_trees(&tokens).map_err(|error| format!("{error:?}"))?;
    let syntax = syntax_trees_to_symbol_resolved_trees::normalize_generic_data(syntax)
        .map_err(|error| format!("{error:?}"))?;
    let resolved = lower_syntax_trees(&syntax).map_err(|error| format!("{error:?}"))?;
    let typed = lower_symbol_resolved_trees(&resolved).map_err(|error| format!("{error:?}"))?;
    lower_typed_trees(typed).map_err(|error| format!("{error:?}"))
}

fn accepts(source: &str) {
    check(source).unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics}"));
}

fn rejects_anonymous_remainder(source: &str) {
    let diagnostics = match check(source) {
        Ok(_) => panic!("anonymous remainder acquired integer meaning: {source}"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics.contains('%')
            && diagnostics.contains("integer")
            && diagnostics.contains("operand"),
        "expected the missing integer-typed operand diagnostic: {source}\n{diagnostics:#?}"
    );
}

#[test]
fn a_return_destination_does_not_type_anonymous_remainder_operands() {
    for (expression, _) in ANONYMOUS_REMAINDERS {
        rejects_anonymous_remainder(&format!("machine run() -> i32 {{ {expression} }}"));
    }
}

#[test]
fn a_local_destination_does_not_type_anonymous_remainder_operands() {
    for (expression, _) in ANONYMOUS_REMAINDERS {
        rejects_anonymous_remainder(&format!(
            "machine run() -> i32 {{ let value: i32 = {expression}; value }}"
        ));
    }
}

#[test]
fn a_call_parameter_does_not_type_anonymous_remainder_operands() {
    for (expression, _) in ANONYMOUS_REMAINDERS {
        rejects_anonymous_remainder(&format!(
            "machine take(value: i32) -> i32 {{ value }}
            machine run() -> i32 {{ take({expression}) }}"
        ));
    }
}

#[test]
fn a_range_bound_cannot_fold_anonymous_remainder_into_an_integer() {
    for (expression, _) in ANONYMOUS_REMAINDERS {
        // The enclosing range is otherwise valid for every result, including
        // the negative dividend, so invalid range geometry cannot mask this.
        rejects_anonymous_remainder(&format!(
            "machine run() -> i32 [0..=(({expression}) + 10)] {{ 0 }}"
        ));
    }
}

#[test]
fn requirements_do_not_supply_an_integer_operand_type_for_remainder() {
    for (expression, result) in ANONYMOUS_REMAINDERS {
        rejects_anonymous_remainder(&format!(
            "machine run() -> i32 requires ({expression}) == {result}; {{ 0 }}"
        ));
    }
}

#[test]
fn guarantees_do_not_supply_an_integer_operand_type_for_remainder() {
    for (expression, result) in ANONYMOUS_REMAINDERS {
        rejects_anonymous_remainder(&format!(
            "machine run() -> i32 ensures ({expression}) == {result}; {{ 0 }}"
        ));
    }
}

#[test]
fn an_explicit_integer_operand_preserves_remainder_in_each_context() {
    for (expression, result) in [("7i32 % 2", "1"), ("8i32 % 2", "0"), ("-3i32 % 2", "-1")] {
        accepts(&format!("machine run() -> i32 {{ {expression} }}"));
        accepts(&format!(
            "machine run() -> i32 {{ let value: i32 = {expression}; value }}"
        ));
        accepts(&format!(
            "machine take(value: i32) -> i32 {{ value }}
            machine run() -> i32 {{ take({expression}) }}"
        ));
        accepts(&format!(
            "machine run() -> i32 [0..=(({expression}) + 10)] {{ 0 }}"
        ));
        accepts(&format!(
            "machine run() -> i32 requires ({expression}) == {result}; {{ 0 }}"
        ));
        // Typed remainder evaluation in the proof solver is a separate open
        // slice. This control preserves fact transport across both contexts
        // without asking that solver to compute the remainder from scratch.
        accepts(&format!(
            "machine run() -> i32
                requires ({expression}) == {result};
                ensures ({expression}) == {result};
            {{ 0 }}"
        ));
    }
}

#[test]
fn either_integer_operand_can_select_remainder_without_a_destination_hint() {
    accepts("machine run() -> u32 { 7u32 % 2 }");
    accepts("machine run() -> u32 { 7 % 2u32 }");
    accepts("machine run(value: u32) -> u32 { value % 2 }");
    accepts("machine run(value: i32) -> i32 { value % 2 }");
}

#[test]
fn a_const_generic_argument_cannot_erase_anonymous_remainder_before_checking() {
    for expression in ["7 % 2", "8 % 2"] {
        rejects_anonymous_remainder(&format!(
            "data Buffer<const N: u64> {{ items: [u8; N]; }}
            machine run(value: &Buffer<{expression}>) {{}}"
        ));
    }
    accepts(
        "data Buffer<const N: u64> { items: [u8; N]; }
        machine run(value: &Buffer<7u64 % 2>) {}",
    );
}

#[test]
fn exact_const_quotients_determine_checked_array_bounds() {
    for (argument, last_element) in [("7 / 2 * 2", 6), ("7u64 / 2 * 2", 5)] {
        let source = format!(
            "data Buffer<const N: u64> {{ values: [u8; N]; }}
            machine run(value: &Buffer<{argument}>) -> u8 {{ value.values[{last_element}] }}"
        );
        accepts(&source);
        let outside = source.replace(
            &format!("values[{last_element}]"),
            &format!("values[{}]", last_element + 1),
        );
        let errors = check(&outside).expect_err("the first out-of-range element must reject");
        assert!(errors.contains("index"), "{outside}: {errors}");
    }
}

#[test]
fn const_requirements_reach_checking_with_exact_anonymous_meaning() {
    for fact in [
        "7 / 2 * 2 == 7",
        "7 / 2 > 3",
        "7u64 / 2 == 3",
        "N == 7 / 2 * 2 - 5",
    ] {
        accepts(&format!(
            "data Buffer<const N: u64> where {fact}, {{ values: [u8; N]; }}
            machine run(value: &Buffer<2>) -> u8 {{ value.values[0] }}"
        ));
    }
    let source = "data Buffer<const N: u64> where 7 / 2 == 3, { values: [u8; N]; }
        machine run(value: &Buffer<2>) -> u8 { value.values[0] }";
    let errors = check(source).expect_err("a false const requirement cannot be erased");
    assert!(errors.contains("is false"), "{errors}");
}

#[test]
fn template_deferral_does_not_admit_unsupported_concrete_field_facts() {
    for source in [
        "data Buffer<const N: u64> where count / 2 <= N, { count: u64; }
         machine run(value: &Buffer<2>) -> u64 { value.count }",
        "data Buffer where count / 2 <= 2, { count: u64; }
         machine run(value: &Buffer) -> u64 { value.count }",
    ] {
        let errors = check(source)
            .expect_err("concrete zero classification still rejects unsupported facts");
        assert!(errors.contains("zero-foldable fragment"), "{errors}");
    }
}

#[test]
fn nested_anonymous_arithmetic_does_not_supply_a_remainder_operand_type() {
    for expression in ["(7 + 1) % 2", "(7 / 2) % 2", "(7 % 2) as i32"] {
        rejects_anonymous_remainder(&format!("machine run() -> i32 {{ {expression} }}"));
    }
    accepts("machine run() -> i32 { (7i32 + 1) % 2 }");
}

#[test]
fn assignment_does_not_supply_a_remainder_operand_type() {
    for (expression, _) in ANONYMOUS_REMAINDERS {
        rejects_anonymous_remainder(&format!(
            "machine run() -> i32 {{ let mut value: i32 = 0; value = {expression}; value }}"
        ));
    }
    accepts("machine run() -> i32 { let mut value: i32 = 0; value = -3i32 % 2; value }");
}

#[test]
fn authored_operator_meanings_remain_independent_typing_boundaries() {
    accepts(
        "operator % Math::remainder(left: i32, right: i32) -> i32;
        machine run() -> i32 { 7 % 2 }",
    );
    accepts(
        "operator + Math::add(left: i32, right: i32) -> i32;
        machine run() -> i32 { (7 + 1) % 2 }",
    );
}
