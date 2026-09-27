use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program_result;

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    lower_typed_trees(typed_program_result(source)?, &CheckingRequest::settled())
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
    let mut program =
        typed_program_result("data Authored { value: u8; }").expect("typed authored data");
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
