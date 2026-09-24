use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::ContractProofFactKind;
use crate::tests::contracts::parse_typed_trees;

#[test]
fn checked_proposition_declarations_retain_public_visibility_without_minting_facts() {
    let source = r#"
        pub proposition visible();
        proposition hidden();
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect("proposition visibility should survive checked lowering");
    let declarations = &checked.facts.proof.proposition_vocabulary.declarations;
    assert_eq!(declarations.len(), 2);
    assert!(
        declarations
            .iter()
            .any(|declaration| declaration.name == "visible" && declaration.is_public)
    );
    assert!(
        declarations
            .iter()
            .any(|declaration| declaration.name == "hidden" && !declaration.is_public)
    );
    assert!(
        checked
            .facts
            .proof
            .proposition_vocabulary
            .applications
            .is_empty()
    );
}

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

    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect("the concrete type argument should instantiate the proposition value signature");
}

#[test]
fn carrierless_evidence_projection_cannot_select_an_executable_machine_parameter() {
    let source = r#"
        trait Evidence {
            machine modulus() -> i32;
        }

        proposition holds() evidence Evidence;

        machine consume<machine Witness>()
        where machine Witness() -> i32;
        {}

        machine caller()
        requires proof: holds()
        {
            consume<proof.modulus>();
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("erased evidence must not become an executable callback");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "proof-static evidence projection `proof.modulus` cannot select an executable machine parameter",
        )
    }));
}

#[test]
fn carrierless_evidence_projection_binds_the_exact_term_and_requirement_row() {
    let source = r#"
        trait Evidence {
            machine modulus() -> i32;
        }

        proposition holds() evidence Evidence;
        proposition selected<machine Witness>();

        machine caller()
        requires proof: holds()
        requires selected<proof.modulus>()
        {
        }
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect("a proof-static projection should bind to checked evidence");
    let projection = checked
        .facts
        .proof
        .proposition_vocabulary
        .applications
        .iter()
        .flat_map(|application| &application.binder_arguments)
        .find_map(|argument| argument.evidence_projection.as_ref())
        .expect("the checked proposition argument should retain a structured projection");
    assert_eq!(
        checked.facts.proof.evidence_terms.get(projection.term).name,
        "proof"
    );
    assert_eq!(checked.symbols.name(projection.requirement), "modulus");
    assert_eq!(checked.symbols.name(projection.declaring_trait), "Evidence");
}

#[test]
fn carrierless_evidence_projection_rejects_an_unknown_requirement() {
    let source = r#"
        trait Evidence {
            machine modulus() -> i32;
        }

        proposition holds() evidence Evidence;
        proposition selected<machine Witness>();

        machine caller()
        requires proof: holds()
        requires selected<proof.missing>()
        {
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("an unknown proof-static requirement must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "carrierless evidence interface `Evidence` does not contain `missing` for projection `proof.missing`",
        )
    }));
}

#[test]
fn carrierless_evidence_projection_rejects_an_ambiguous_inherited_requirement() {
    let source = r#"
        trait First {
            machine modulus() -> i32;
        }
        trait Second {
            machine modulus() -> i32;
        }
        trait Evidence: First + Second {}

        proposition holds() evidence Evidence;
        proposition selected<machine Witness>();

        machine caller()
        requires proof: holds()
        requires selected<proof.modulus>()
        {
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("an ambiguous inherited proof-static requirement must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "carrierless evidence interface `Evidence` contains more than one requirement named `modulus` for projection `proof.modulus`",
        )
    }));
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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
            output = first;
        }
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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

    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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

    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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

    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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

    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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
    assert_eq!(forwarding.statement_index, 0);
    let checked_trees::EvidenceAssignmentSource::Forwarded { term: source } = &forwarding.source
    else {
        panic!("an incoming evidence assignment must retain forwarding identity")
    };
    assert_eq!(
        checked.facts.proof.evidence_terms.get(*source).name,
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
        checked.facts.proof.evidence_terms.get(*source).proposition,
        checked
            .facts
            .proof
            .evidence_terms
            .get(forwarding.output)
            .proposition
    );
}
