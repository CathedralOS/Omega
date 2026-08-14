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

#[test]
fn named_requires_arguments_bind_exact_checked_terms_by_position() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;

        machine consume(value: i32)
        requires required: carries(value)
        {
        }

        machine forward(value: i32)
        requires incoming: carries(value)
        {
            consume(value; incoming);
        }
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source))
        .expect("an explicit matching evidence term should satisfy the erased call lane");
    let call = checked
        .facts
        .proof
        .contract_calls
        .iter()
        .map(|(_, call)| call)
        .find(|call| !call.evidence_arguments.is_empty())
        .expect("checked call evidence binding");
    let [binding] = checked
        .facts
        .proof
        .contract_evidence_arguments
        .span_or_empty(call.evidence_arguments)
    else {
        panic!("one positional evidence binding expected");
    };
    let source = checked.facts.proof.evidence_terms.get(binding.source);
    let parameter = checked.facts.proof.evidence_terms.get(binding.parameter);
    assert_eq!(source.name, "incoming");
    assert_eq!(parameter.name, "required");
    assert_eq!(binding.lane_position, 0);
}

#[test]
fn expression_call_binds_named_requires_evidence_lane() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;

        machine consume(value: i32) -> i32
        requires required: carries(value)
        {
            value
        }

        machine forward(value: i32) -> i32
        requires incoming: carries(value)
        {
            let result: i32 = consume(value; incoming);
            result
        }
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source))
        .expect("value calls must bind the same checked evidence lane as statement calls");
    let call = checked
        .facts
        .proof
        .contract_calls
        .iter()
        .map(|(_, call)| call)
        .find(|call| !call.evidence_arguments.is_empty())
        .expect("checked expression-call evidence binding");
    let [binding] = checked
        .facts
        .proof
        .contract_evidence_arguments
        .span_or_empty(call.evidence_arguments)
    else {
        panic!("one positional expression-call evidence binding expected");
    };
    assert_eq!(
        checked.facts.proof.evidence_terms.get(binding.source).name,
        "incoming"
    );
}

#[test]
fn evidence_only_call_binds_after_leading_semicolon() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;

        machine consume()
        requires required: ready()
        {
        }

        machine forward()
        requires incoming: ready()
        {
            consume(; incoming);
        }
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source))
        .expect("the leading semicolon must distinguish an evidence-only call lane");
    assert_eq!(checked.facts.proof.contract_evidence_arguments.len(), 1);
}

#[test]
fn forwarding_named_requires_to_ensures_preserves_exact_term_identity() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;

        machine forward(value: i32)
        requires incoming: carries(value)
        ensures outgoing: carries(value)
        {
            outgoing = incoming;
        }
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source))
        .expect("matching named evidence terms should forward");
    let forwardings = checked
        .facts
        .proof
        .evidence_forwardings
        .iter()
        .map(|(_, forwarding)| forwarding)
        .collect::<Vec<_>>();
    let [forwarding] = forwardings.as_slice() else {
        panic!("one checked forwarding expected");
    };
    assert_eq!(
        checked
            .facts
            .proof
            .evidence_terms
            .get(forwarding.source)
            .name,
        "incoming"
    );
    assert_eq!(
        checked
            .facts
            .proof
            .evidence_terms
            .get(forwarding.output)
            .name,
        "outgoing"
    );
    assert_eq!(
        checked
            .facts
            .proof
            .evidence_terms
            .get(forwarding.source)
            .proposition,
        checked
            .facts
            .proof
            .evidence_terms
            .get(forwarding.output)
            .proposition
    );
}

#[test]
fn evidence_forwarding_rejects_unknown_source() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;
        machine forward(value: i32)
        requires incoming: carries(value)
        ensures outgoing: carries(value)
        {
            outgoing = absent;
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("forwarding must name an exact incoming term");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("source `absent` is not a named requires binding")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn evidence_forwarding_rejects_assignment_to_incoming_term() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;
        machine forward(value: i32)
        requires incoming: carries(value)
        {
            incoming = incoming;
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("incoming evidence aliases are immutable inputs");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("target `incoming` is not a named ensures binding")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn evidence_forwarding_rejects_proposition_mismatch() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;
        proposition differs(value: i32) evidence Evidence;
        machine forward(value: i32)
        requires differs(value)
        requires incoming: carries(value)
        ensures outgoing: differs(value)
        {
            outgoing = incoming;
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("forwarding cannot change proposition identity");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("because their proposition identities differ")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn named_requires_call_rejects_ambient_fact_inference() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;

        machine consume(value: i32)
        requires required: carries(value)
        {
        }

        machine forward(value: i32)
        requires incoming: carries(value)
        {
            consume(value);
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("a visible matching fact must not synthesize an erased argument");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("supplies 0 erased evidence arguments but its named requires lane has 1")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn named_requires_call_rejects_wrong_proposition_term() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;
        proposition differs(value: i32) evidence Evidence;

        machine consume(value: i32)
        requires required: carries(value)
        {
        }

        machine forward(value: i32)
        requires carries(value)
        requires incoming: differs(value)
        {
            consume(value; incoming);
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("an explicit term of another proposition must not bind by name or visibility");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not inhabit erased requires position 0")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn erased_call_lane_rejects_extra_terms_for_unnamed_callee() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;

        machine consume(value: i32) {
        }

        machine forward(value: i32)
        requires incoming: carries(value)
        {
            consume(value; incoming);
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("an erased argument cannot be silently dropped");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("supplies 1 erased evidence argument but its named requires lane has 0")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn erased_call_lane_rejects_unknown_source_term() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;

        machine consume(value: i32)
        requires required: carries(value)
        {
        }

        machine forward(value: i32)
        requires incoming: carries(value)
        {
            consume(value; absent);
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("an evidence-lane name must resolve to a caller requires term");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("unknown incoming evidence term `absent`")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}
