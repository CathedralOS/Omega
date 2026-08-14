use super::*;

#[test]
fn proposition_type_arguments_instantiate_value_parameter_types() {
    let source = r#"
        proposition typed<T>(value: T);
        data Main { value: i32; }

        machine Main::run(&mut self)
        requires typed<i32>(self.value)
        {
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("the concrete type argument should instantiate the proposition value signature");
}

#[test]
fn proposition_type_arguments_reject_mismatched_value_arguments() {
    let source = r#"
        proposition typed<T>(value: T);
        data Main { value: bool; }

        machine Main::run(&mut self)
        requires typed<i32>(self.value)
        {
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("a bool value cannot satisfy a proposition parameter instantiated as i32");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "proposition `typed` argument 1 does not match parameter `value` type `i32`",
            )
        }),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn named_witness_contracts_mint_distinct_positional_checked_terms() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;
        proposition forwarded(value: i32) = carries(value);

        machine consume(value: i32)
        requires first: forwarded(value)
        requires second: carries(value)
        ensures output: carries(value)
        {
        }
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source))
        .expect("named witness contracts should lower to checked evidence terms");
    let terms = checked
        .facts
        .proof
        .evidence_terms
        .iter()
        .collect::<Vec<_>>();

    assert_eq!(terms.len(), 3);
    assert_ne!(
        terms[0].0, terms[1].0,
        "each binding has exact term identity"
    );
    assert_ne!(
        terms[1].0, terms[2].0,
        "each binding has exact term identity"
    );
    assert_eq!(terms[0].1.name, "first");
    assert_eq!(terms[0].1.lane_position, 0);
    assert_eq!(terms[1].1.name, "second");
    assert_eq!(terms[1].1.lane_position, 1);
    assert_eq!(terms[2].1.name, "output");
    assert_eq!(terms[2].1.lane_position, 0);
    assert_eq!(terms[0].1.kind, ContractProofFactKind::Requires);
    assert_eq!(terms[2].1.kind, ContractProofFactKind::Ensures);
    assert_eq!(terms[0].1.proposition, terms[1].1.proposition);
    assert_eq!(terms[1].1.proposition, terms[2].1.proposition);
    assert_eq!(terms[0].1.evidence_type, "Evidence");

    let carries = checked
        .typed
        .propositions()
        .iter()
        .find(|proposition| proposition.name.as_str() == "carries")
        .expect("nominal witness endpoint");
    assert_eq!(terms[0].1.proposition.declaration, carries.symbol);

    let bound_terms = checked
        .facts
        .proof
        .contract_facts
        .iter()
        .filter_map(|(_, fact)| fact.evidence_term)
        .collect::<Vec<_>>();
    assert_eq!(bound_terms, vec![terms[0].0, terms[1].0, terms[2].0]);
}
