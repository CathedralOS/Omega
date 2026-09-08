use super::*;

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    lower_typed_trees(typed(source)?)
}

fn typed(source: &str) -> Result<typed_trees::TypedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("lex fixture");
    let syntax = parse_syntax_trees(&tokens).expect("parse fixture");
    let resolved = lower_syntax_trees(&syntax)?;
    lower_symbol_resolved_trees(&resolved).map_err(|diagnostic| vec![diagnostic])
}

#[test]
fn embedding_is_an_exact_builtin_without_a_machine_declaration() {
    let checked = check(
        r#"
        machine arithmetic(value: u64)
        ensures embed(value) + 1 > embed(value)
        {}
        "#,
    )
    .expect("unbounded proof arithmetic");
    assert!(
        checked
            .machines()
            .iter()
            .all(|machine| machine.name.as_str() != "embed")
    );
}

#[test]
fn signed_integer_remainders_cannot_prove_unsigned_bounds() {
    for dividend in ["value", "embed(value)"] {
        let source = format!(
            "machine remainder(value: i32)\nrequires value == -7\nensures {dividend} % 2 >= 0\n{{}}"
        );
        assert!(
            check(&source).is_err(),
            "false remainder contract: {source}"
        );
    }
}

#[test]
fn integer_remainder_proof_bounds_preserve_the_dividend_sign() {
    for dividend in ["value", "embed(value)"] {
        for divisor in ["2", "-2"] {
            for (carrier, requirement, conclusion) in [
                ("i32", "", ">= -1"),
                ("i32", "", "<= 1"),
                ("i32", "requires value <= 0", "<= 0"),
                ("i32", "requires value >= 0", ">= 0"),
            ] {
                let source = format!(
                    "machine remainder(value: {carrier})\n{requirement}\nensures {dividend} % {divisor} {conclusion}\n{{}}"
                );
                check(&source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:?}"));
            }
        }
        let source = format!("machine remainder(value: u64)\nensures {dividend} % 2 >= 0\n{{}}");
        check(&source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:?}"));
    }
}

#[test]
fn embedding_rejects_boolean_and_comparison_results() {
    for expression in ["true", "value == value", "value < value", "value != value"] {
        let source = format!("machine predicate(value: u8) ensures embed({expression}) == 0 {{}}");
        let diagnostics = check(&source).expect_err("Boolean results are not integer payloads");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("embed")),
            "{expression}: {diagnostics:?}"
        );
    }
}

#[test]
fn embedding_rejects_noninteger_carriers_and_runtime_use() {
    for source in [
        "machine predicate(value: f64) ensures embed(value) == 0 {}",
        "machine escape(value: u64) -> u64 { embed(value); value }",
        "machine escape(value: u64) -> u64 { embed(value) as u64 }",
    ] {
        let diagnostics = check(source).expect_err("embedding is integer-only and proof-only");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("embed")),
            "{source}: {diagnostics:?}"
        );
    }
}

#[test]
fn embedding_a_trapping_binding_does_not_form_trapping_arithmetic() {
    check("machine predicate(value: i32 in Trapping) ensures embed(value) == embed(value) {}")
        .expect("embedding is total independently of binding policy");
    check("machine predicate(value: i32 in Trapping) ensures embed(value + 1) == 0 {}")
        .expect_err("nested trapping arithmetic is not a total proof term");
}

#[test]
fn embedding_requires_one_value_and_no_static_arguments() {
    for expression in ["embed()", "embed(value, value)", "embed<u8>(value)"] {
        let source = format!("machine predicate(value: u8) ensures {expression} == 0 {{}}");
        check(&source).expect_err("only the closed unary proof term is admitted");
    }
}

#[test]
fn an_authored_machine_cannot_replace_integer_embedding() {
    assert!(check(
        "machine embed(value: u8) -> u8 { value } machine predicate(value: u8) ensures embed(value) == value {}",
    )
    .is_err(), "authored machine identity is not the compiler projection");
}

#[test]
fn a_computed_proof_machine_returns_an_integer_embedding_without_runtime_call_storage() {
    check("machine payload(value: i32) -> Int { embed(value) }")
        .expect("computed proof machine returns mathematical payload");
}

#[test]
fn natural_coercion_uses_prior_nonnegativity_for_signed_payloads() {
    let source = r#"
        data Nat { case Zero; case Succ(previous: Nat); }
        machine nonnegative(value: i32) -> Nat
        requires value >= 0
        { embed(value) as Nat }
    "#;
    check(source).expect("prior signed nonnegativity permits proof-only coercion");
    check(&source.replace("requires value >= 0", ""))
        .expect_err("signed carrier range alone does not establish nonnegativity");
}

#[test]
fn unsigned_embedding_subtraction_is_signed_until_proven_nonnegative() {
    let source = r#"
        data Nat { case Zero; case Succ(previous: Nat); }
        machine distance(start: u64, end: u64) -> Nat
        requires end >= start
        { (embed(end) - embed(start)) as Nat }
    "#;
    check(source).expect("ordered endpoints permit exact natural distance");
    check(&source.replace("requires end >= start", ""))
        .expect_err("unsigned payloads do not make their mathematical difference nonnegative");
}

#[test]
fn a_later_or_enclosing_fact_cannot_justify_natural_coercion_formation() {
    let source = r#"
        data Nat { case Zero; case Succ(previous: Nat); }
        machine predicate(value: i32)
        requires
            (embed(value) as Nat) == (embed(value) as Nat),
            value >= 0
        {}
    "#;
    let diagnostics = match check(source) {
        Err(diagnostics) => diagnostics,
        Ok(_) => panic!("later facts cannot form earlier terms"),
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("previously proven nonnegative") }),
        "{diagnostics:?}"
    );
}

#[test]
fn proof_integer_classification_uses_builtin_identity_not_type_spelling() {
    let mut program = typed("data Authored { value: u8; }").expect("typed authored data");
    let authored = program.data_definitions()[0].symbol;
    let builtin = program
        .symbols
        .builtin_type_symbol(symbols::BuiltinType::Int)
        .expect("compiler-installed proof integer");
    let integer =
        program
            .type_reference_table
            .insert(typed_trees::types::TypeReferenceNode::Named {
                symbol: builtin,
                name: typed_trees::name::Identifier::generated("Int"),
            });
    // Retaining the same diagnostic spelling with an authored symbol must not
    // acquire the builtin's classification or allow proof-only consumption.
    let same_spelling =
        program
            .type_reference_table
            .insert(typed_trees::types::TypeReferenceNode::Named {
                symbol: authored,
                name: typed_trees::name::Identifier::generated("Int"),
            });
    let classification = typed_trees::proof_only::classify(&program);
    assert!(classification.is_proof_only(builtin));
    assert!(!classification.is_proof_only(authored));
    assert!(
        classification
            .proof_only_mention(&program, integer)
            .is_some()
    );
    assert!(
        classification
            .proof_only_mention(&program, same_spelling)
            .is_none()
    );
    assert!(
        check("data Int { value: u8; }").is_err(),
        "source cannot replace the builtin"
    );
}

#[test]
fn proof_integer_inline_containment_propagates_but_erasure_and_indirection_do_not() {
    let program = typed(
        r#"
        data Direct { value: Int; }
        data Wrapper { inner: Direct; }
        data ArrayHolder { values: [Int; 2]; }
        data CaseHolder { case Present(value: Int); case Absent; }
        data Erased { value [erased]: Int; }
        data Borrowed { value: &Int; }
        data Sliced { values: [Int]; }
    "#,
    )
    .expect("type classification fixture");
    let classification = typed_trees::proof_only::classify(&program);
    for definition in program.data_definitions() {
        let expected = matches!(
            definition.name.as_str(),
            "Direct" | "Wrapper" | "ArrayHolder" | "CaseHolder"
        );
        assert_eq!(
            classification.is_proof_only(definition.symbol),
            expected,
            "classification of {}",
            definition.name
        );
    }
    // Indirection breaks inline contagion, but observing or storing the
    // resulting reference/slice still mentions a proof-only referent.
    for name in ["Borrowed", "Sliced"] {
        let definition = program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == name)
            .expect("holder");
        let typed_trees::data::DataMember::Field(field) = &program.data_members(definition)[0]
        else {
            panic!("holder field")
        };
        assert!(
            classification
                .proof_only_mention(&program, field.type_reference)
                .is_some()
        );
    }
}

#[test]
fn proof_integer_holders_cannot_claim_runtime_copy_properties() {
    let diagnostics = check("data Holder [copy] { value: Int; }")
        .expect_err("a mathematical integer cannot provide runtime copy storage");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("declares runtime properties")
                && diagnostic.message.contains("proof-only")
        }),
        "{diagnostics:?}"
    );
}

#[test]
fn runtime_holders_cannot_observe_proof_integer_references() {
    let diagnostics = check("data Holder { value: &Int; }")
        .expect_err("proof integers cannot be viewed through runtime indirection");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("views proof-only") && diagnostic.message.contains("Int")
        }),
        "{diagnostics:?}"
    );
}

#[test]
fn integer_embedding_observes_only_its_own_contract_result() {
    check("machine identity(value: u64) -> u64 ensures embed(result) == embed(value) { value }")
        .expect("owning machine result retains its integer carrier");
    check("machine other(value: u64) -> u64 { value } machine predicate() -> bool ensures embed(result) == 0 { true }")
        .expect_err("another machine's integer result cannot type this Boolean result");
    check("machine predicate(result: bool) -> u64 ensures embed(result) == 0 { 0 }")
        .expect_err("a real parameter named result shadows the reserved result");
    check("machine predicate() -> u64 requires embed(result) == 0 { 0 }")
        .expect_err("requires does not observe a not-yet-produced result");
}

#[test]
fn proof_embedding_shift_counts_cannot_be_boolean_or_float_values() {
    for count in ["true", "1.5", "count"] {
        for operator in ["<<", ">>"] {
            let source = format!(
                "machine predicate(value: u8 in Wrapping, count: f64) requires embed(value {operator} {count}) >= 0 {{}}"
            );
            let diagnostics = check(&source).expect_err("shift count must be an integer");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("embed")),
                "{source}: {diagnostics:?}"
            );
        }
    }
}

#[test]
fn closed_proof_integer_quotients_and_remainders_preserve_all_signs() {
    for (dividend, divisor, quotient, remainder) in [
        (7, 2, 3, 1),
        (-7, 2, -3, -1),
        (7, -2, -3, 1),
        (-7, -2, 3, -1),
    ] {
        for (operator, expected) in [("/", quotient), ("%", remainder)] {
            let expression = format!("embed({dividend}i32) {operator} {divisor}");
            let source = format!("machine predicate() ensures {expression} == {expected} {{}}");
            check(&source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:?}"));
            let false_twin = format!(
                "machine predicate() ensures {expression} == {} {{}}",
                expected + 1
            );
            check(&false_twin).expect_err("a neighboring integer is not the quotient or remainder");
        }
    }
}

#[test]
fn closed_proof_integer_arithmetic_exceeds_host_integer_widths() {
    let power_of_two = "(embed(18446744073709551615u64) + 1)";
    let huge = format!("({power_of_two} * {power_of_two} * 2)");
    for conclusion in [
        format!("({huge} + 1) / {huge} == 1"),
        format!("({huge} + 1) % {huge} == 1"),
        format!("(-{huge} - 1) / {huge} == -1"),
        format!("(-{huge} - 1) % {huge} == -1"),
    ] {
        let source = format!("machine predicate() ensures {conclusion} {{}}");
        check(&source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:?}"));
        check(&source.replace("==", "!=")).expect_err("large exact arithmetic false twin");
    }
}

#[test]
fn undefined_proof_integer_arithmetic_cannot_be_hidden_by_normalization() {
    for operator in ["/", "%"] {
        for (parameters, requirement, divisor) in [
            ("", "", "0"),
            ("value: i32", "requires value == 0", "embed(value)"),
        ] {
            let undefined = format!("(embed(7i32) {operator} {divisor})");
            for conclusion in [
                format!("{undefined} == 0"),
                format!("{undefined} == {undefined}"),
                format!("0 * {undefined} == 0"),
                format!("{undefined} - {undefined} == 0"),
            ] {
                let source = format!(
                    "machine predicate({parameters})\n{requirement}\nensures {conclusion} {{}}"
                );
                check(&source).unwrap_err();
            }
        }
    }
}

#[test]
fn proof_integer_folding_preserves_anonymous_and_fixed_width_controls() {
    check("machine rational() ensures 7 / 2 == 3.5 {}")
        .expect("anonymous division remains exact rational arithmetic");
    check("machine rational() ensures 7 / 2 == 3 {}")
        .expect_err("anonymous division does not truncate");
    check("machine anonymous() ensures 7 % 2 == 1 {}")
        .expect_err("proof context does not supply an integer operand");
    check("machine exact() ensures embed(255u8 + 1) == 256 {}")
        .expect_err("embedding cannot erase fixed-width Exact overflow");
    check("machine trapping(value: i32 in Trapping) requires value == 7\nensures embed(value / 2) == 3 {}")
        .expect_err("embedding cannot erase trapping arithmetic formation");
}

#[test]
fn proof_integer_operands_accept_exact_anonymous_arithmetic() {
    for expression in [
        "embed(7i32) / (1 + 1) == 3",
        "embed(7i32) % (1 + 1) == 1",
        "(5 + 2) / embed(2i32) == 3",
        "(5 + 2) % embed(2i32) == 1",
        "embed(7i32) / (1 / 2 * 4) == 3",
        "embed(7i32) % (1 / 2 * 4) == 1",
        "embed(7i32) / (1.5 + 0.5) == 3",
    ] {
        let source = format!("machine predicate()\nensures {expression}\n{{}}");
        check(&source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:?}"));
        check(&source.replace("==", "!=")).expect_err("exact anonymous operand false twin");
    }
}

#[test]
fn proof_integer_operands_do_not_truncate_fractions_or_erase_zero_divisors() {
    for operator in ["/", "%"] {
        for operand in ["(1 / 2)", "(1.5 + 1)", "(1 - 1)"] {
            let expression = format!("(embed(7i32) {operator} {operand})");
            let source = format!("machine predicate()\nensures {expression} == {expression}\n{{}}");
            check(&source).expect_err("an undefined Int operand cannot prove reflexivity");
        }
    }
}

#[test]
fn proof_integer_quotient_bounds_follow_truncation_and_divisor_sign() {
    for (requirement, divisor, minimum, maximum) in [
        ("value >= 0, value <= 9", 2, 0, 4),
        ("value >= -9, value <= -1", 2, -4, 0),
        ("value >= -9, value <= 9", -2, -4, 4),
        ("value >= 1, value <= 9", -2, -4, 0),
    ] {
        let source = format!(
            "machine quotient(value: i32)\nrequires {requirement}\nensures embed(value) / {divisor} >= {minimum}, embed(value) / {divisor} <= {maximum}\n{{}}"
        );
        check(&source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:?}"));
        let false_twin = source.replace(&format!("<= {maximum}"), &format!("<= {}", maximum - 1));
        check(&false_twin).expect_err("a quotient bound cannot exclude a reachable endpoint");
    }
}

#[test]
fn proof_integer_quotient_bounds_preserve_missing_endpoints() {
    for (requirement, divisor, conclusion, false_conclusion) in [
        ("value >= 1", 2, ">= 0", ">= 1"),
        ("value >= 1", -2, "<= 0", "<= -1"),
        ("value <= -1", 2, "<= 0", "<= -1"),
        ("value <= -1", -2, ">= 0", ">= 1"),
    ] {
        for (conclusion, accepted) in [(conclusion, true), (false_conclusion, false)] {
            let source = format!(
                "machine quotient(value: i32)\nrequires {requirement}\nensures embed(value) / {divisor} {conclusion}\n{{}}"
            );
            assert_eq!(check(&source).is_ok(), accepted, "{source}");
        }
    }
}

#[test]
fn proof_integer_quotient_bounds_remain_unbounded_in_width() {
    let wide = "((embed(18446744073709551615u64) + 1) * (embed(18446744073709551615u64) + 1))";
    let source = format!(
        "machine quotient(value: i32)\nrequires value >= 0, value <= 9\nensures (embed(value) + {wide}) / 2 >= {wide} / 2, (embed(value) + {wide}) / 2 <= {wide} / 2 + 4\n{{}}"
    );
    check(&source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:?}"));
    check(&source.replace("+ 4", "+ 3")).expect_err("wide quotient upper bound is reachable");
}

#[test]
fn proof_integer_quotient_bounds_keep_operands_distinct() {
    for conclusion in [
        "embed(left) / 2 == embed(left) / 3",
        "embed(left) / 2 == embed(right) / 2",
        "embed(left) / (1 - 1) == 0",
        "embed(left) / embed(right) == 0",
    ] {
        let source = format!(
            "machine quotient(left: i32, right: i32)\nrequires left >= 0, left <= 9, right >= 1, right <= 3\nensures {conclusion}\n{{}}"
        );
        check(&source)
            .expect_err("distinct quotients or undefined division cannot prove this claim");
    }
}

#[test]
fn proof_integer_remainder_bounds_use_a_known_divisor() {
    for divisor in [2, -2] {
        for (requirement, conclusion) in [
            ("", ">= -1"),
            ("", "<= 1"),
            (", value >= 0", ">= 0"),
            (", value <= 0", "<= 0"),
        ] {
            let source = format!(
                "machine remainder(value: i32, divisor: i32)\nrequires divisor == {divisor}{requirement}\nensures embed(value) % embed(divisor) {conclusion}\n{{}}"
            );
            check(&source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:?}"));
        }
    }
    for (requirement, conclusion) in [
        ("divisor == 2", ">= 0"),
        ("divisor == -2", "<= 0"),
        ("divisor == 0", "== 0"),
        ("divisor >= 2", "<= 1"),
        ("divisor == 3", "<= 1"),
    ] {
        let source = format!(
            "machine remainder(value: i32, divisor: i32)\nrequires {requirement}\nensures embed(value) % embed(divisor) {conclusion}\n{{}}"
        );
        assert!(
            check(&source).is_err(),
            "false remainder contract: {source}"
        );
    }
}
