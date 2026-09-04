use super::*;

fn check(
    source: &str,
) -> Result<psi_checked_trees::CheckedTrees, Vec<psi_diagnostics::Diagnostic>> {
    lower_typed_trees(typed(source)?)
}

fn typed(source: &str) -> Result<psi_typed_trees::TypedTrees, Vec<psi_diagnostics::Diagnostic>> {
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
        .builtin_type_symbol(psi_symbols::BuiltinType::Int)
        .expect("compiler-installed proof integer");
    let integer =
        program
            .type_reference_table
            .insert(psi_typed_trees::types::TypeReferenceNode::Named {
                symbol: builtin,
                name: psi_typed_trees::name::Identifier::generated("Int"),
            });
    // Retaining the same diagnostic spelling with an authored symbol must not
    // acquire the builtin's classification or allow proof-only consumption.
    let same_spelling =
        program
            .type_reference_table
            .insert(psi_typed_trees::types::TypeReferenceNode::Named {
                symbol: authored,
                name: psi_typed_trees::name::Identifier::generated("Int"),
            });
    let classification = psi_typed_trees::proof_only::classify(&program);
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
    let classification = psi_typed_trees::proof_only::classify(&program);
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
        let psi_typed_trees::data::DataMember::Field(field) = &program.data_members(definition)[0]
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
