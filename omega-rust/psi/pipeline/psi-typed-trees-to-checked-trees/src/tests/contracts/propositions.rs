use super::*;

#[test]
fn checked_proposition_declarations_retain_public_visibility_without_minting_facts() {
    let source = r#"
        pub proposition visible();
        proposition hidden();
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source))
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

    lower_typed_trees(parse_typed_trees(source))
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
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

    let checked = lower_typed_trees(parse_typed_trees(source))
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
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
            output = first;
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
    assert_eq!(forwarding.statement_index, 0);
    let psi_checked_trees::EvidenceAssignmentSource::Forwarded { term: source } =
        &forwarding.source
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

#[test]
fn immediate_proof_output_binds_a_fresh_erased_evidence_term() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}

        machine produce()
        ensures outgoing: ready()
        {
            outgoing = ConcreteEvidence;
        }

        machine relay()
        ensures relayed: ready()
        {
            let (; outgoing: local) = produce();
            relayed = local;
        }
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source))
        .expect("the first proof-only output rung should check");
    let [typed_invocation] = checked.proof_output_calls.as_slice() else {
        panic!("one typed proof-output invocation expected")
    };
    let [typed_binding] = typed_invocation.bindings.as_ref() else {
        panic!("one typed proof-output binding expected")
    };
    assert_eq!(typed_binding.output_field.as_str(), "outgoing");
    assert_eq!(typed_binding.binding.as_str(), "local");
    let invocation = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .next()
        .map(|(_, invocation)| invocation)
        .expect("one checked proof-output invocation expected");
    let [output] = invocation.outputs.as_slice() else {
        panic!("one checked proof-output output expected")
    };
    let caller_output = output.output.expect("the field is bound in the caller");
    assert_ne!(caller_output, output.callee_output);
    assert_eq!(
        checked.facts.proof.evidence_terms.get(caller_output).name,
        "local"
    );
    let relay = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "relay")
        .expect("relay machine");
    assert!(checked.machine_states(relay)[0].statement_nodes.is_empty());
    let forwarding = checked
        .facts
        .proof
        .evidence_forwardings
        .iter()
        .find_map(|(_, forwarding)| {
            (forwarding.machine_symbol == relay.symbol).then_some(forwarding)
        })
        .expect("relay output forwarding");
    let psi_checked_trees::EvidenceAssignmentSource::Forwarded { term } = forwarding.source else {
        panic!("the caller-local proof output must forward by exact term identity")
    };
    assert_eq!(term, caller_output);
}

#[test]
fn immediate_proof_output_completely_binds_multiple_fresh_terms() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}

        machine produce()
        ensures first: ready()
        ensures second: ready()
        {
            first = ConcreteEvidence;
            second = ConcreteEvidence;
        }

        machine relay()
        ensures relayed_first: ready()
        ensures relayed_second: ready()
        {
            let (; second: local_second, first: local_first) = produce();
            relayed_first = local_first;
            relayed_second = local_second;
        }
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source))
        .expect("a complete multi-field proof-only call should check");
    let [typed_invocation] = checked.proof_output_calls.as_slice() else {
        panic!("one typed proof-output invocation expected")
    };
    assert_eq!(typed_invocation.bindings.len(), 2);
    let invocation = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .next()
        .map(|(_, invocation)| invocation)
        .expect("one checked proof-output invocation expected");
    let [first, second] = invocation.outputs.as_slice() else {
        panic!("two checked proof-output outputs expected")
    };
    assert_eq!((first.output_position, second.output_position), (0, 1));
    let first_output = first.output.expect("first field is bound");
    let second_output = second.output.expect("second field is bound");
    assert_eq!(
        checked.facts.proof.evidence_terms.get(first_output).name,
        "local_first"
    );
    assert_eq!(
        checked.facts.proof.evidence_terms.get(second_output).name,
        "local_second"
    );
    assert_ne!(first_output, first.callee_output);
    assert_ne!(second_output, second.callee_output);
    assert_ne!(first_output, second_output);

    let relay = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "relay")
        .expect("relay machine");
    assert!(checked.machine_states(relay)[0].statement_nodes.is_empty());
}

#[test]
fn argumented_proof_output_substitutes_value_arguments_and_binds_erased_inputs() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;

        machine produce(value: i32)
        requires incoming: carries(value)
        ensures copied: carries(value)
        {
            copied = incoming;
        }

        machine relay(input: i32)
        requires source: carries(input)
        ensures relayed: carries(input)
        {
            let (; copied: local) = produce(input; source);
            relayed = local;
        }
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source))
        .expect("proof-output calls substitute value arguments and bind exact erased inputs");
    let invocation = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .next()
        .map(|(_, invocation)| invocation)
        .expect("one checked proof-output invocation");
    let [argument] = invocation.evidence_arguments.as_slice() else {
        panic!("one erased proof-output input expected")
    };
    assert_eq!(argument.input_position, 0);
    assert_eq!(argument.instantiated_proposition.arguments, ["input"]);
    assert_eq!(
        checked
            .facts
            .proof
            .evidence_terms
            .get(argument.source)
            .proposition,
        argument.instantiated_proposition
    );
    let [output] = invocation.outputs.as_slice() else {
        panic!("one proof output expected")
    };
    assert_eq!(output.instantiated_proposition.arguments, ["input"]);
    assert_eq!(
        checked
            .facts
            .proof
            .evidence_terms
            .get(output.output.expect("captured output"))
            .proposition,
        output.instantiated_proposition
    );
}

#[test]
fn closed_generic_proof_output_retains_its_concrete_application() {
    let source = r#"
        trait Evidence {}
        trait Marker {}
        proposition ready() evidence Evidence;
        data Card {}
        data Root { card: Card; }
        CardMarker: Card satisfies Marker {}

        machine produce<Element, Selection: Element satisfies Marker>(value: &Element)
        requires incoming: ready()
        ensures copied: ready()
        {
            copied = incoming;
        }

        machine Root::relay(&self)
        requires source: ready()
        ensures relayed: ready()
        {
            let (; copied: local) = produce<Card, CardMarker>(&self.card; source);
            relayed = local;
        }
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source))
        .expect("a fully explicit closed generic proof-output call should check");
    let invocation = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .next()
        .map(|(_, invocation)| invocation)
        .expect("one checked proof-output invocation");
    let [argument] = invocation.evidence_arguments.as_slice() else {
        panic!("one erased proof-output input expected")
    };
    let [output] = invocation.outputs.as_slice() else {
        panic!("one proof output expected")
    };
    let specialization = checked
        .machine_specializations
        .iter()
        .find(|specialization| specialization.instance == invocation.target_machine_symbol)
        .expect("the proof-output target should retain its closed generic specialization");
    assert_eq!(
        specialization.type_argument_identities,
        ["named(name(Card))"]
    );
    assert_eq!(specialization.conformance_applications.len(), 1);
    assert_ne!(
        specialization.conformance_applications[0].report_fingerprint,
        0
    );
    assert_eq!(
        output.instantiated_proposition,
        argument.instantiated_proposition
    );
}

#[test]
fn argumented_proof_output_rejects_wrong_erased_input_after_substitution() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;

        machine produce(value: i32)
        requires incoming: carries(value)
        ensures copied: carries(value)
        {
            copied = incoming;
        }

        machine relay(input: i32, other: i32)
        requires wrong: carries(other)
        {
            let (; copied: local) = produce(input; wrong);
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("the erased input must inhabit the call-substituted proposition");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(
                "does not inhabit erased requires position 0 of proof-output call `produce`"
            )),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn discarded_argumented_proof_output_contributes_the_substituted_fact() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;

        machine produce(value: i32)
        requires incoming: carries(value)
        ensures copied: carries(value)
        {
            copied = incoming;
        }

        machine consume(value: i32)
        requires carries(value)
        {}

        machine relay(input: i32)
        requires source: carries(input)
        {
            let (; copied: _) = produce(input; source);
            consume(input);
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("a discarded output contributes its call-substituted proposition fact");
}

#[test]
fn proof_output_lane_allows_selective_capture() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}
        machine produce()
        ensures first: ready()
        ensures second: ready()
        { first = ConcreteEvidence; second = ConcreteEvidence; }
        machine relay()
        ensures relayed: ready()
        {
            let (; first: local) = produce();
            relayed = local;
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("unmentioned proof outputs contribute facts without minting local terms");
}

#[test]
fn omitted_proof_output_contributes_its_fact_without_a_local_term() {
    let source = r#"
        trait Evidence {}
        proposition first_ready() evidence Evidence;
        proposition second_ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}

        machine produce()
        ensures first: first_ready()
        ensures second: second_ready()
        { first = ConcreteEvidence; second = ConcreteEvidence; }

        machine consume_second()
        requires second_ready()
        {}

        machine relay() {
            let (; first: local_first) = produce();
            consume_second();
        }
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source))
        .expect("an omitted proof selector still contributes its proposition fact");
    let invocation = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .next()
        .map(|(_, invocation)| invocation)
        .expect("one checked proof-output invocation");
    let [first, second] = invocation.outputs.as_slice() else {
        panic!("two callee proof outputs")
    };
    assert!(first.output.is_some());
    assert!(second.output.is_none());
}

#[test]
fn proof_output_rejects_duplicate_fields_and_local_names() {
    let duplicate_field = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}
        machine produce()
        ensures first: ready()
        ensures second: ready()
        { first = ConcreteEvidence; second = ConcreteEvidence; }
        machine relay()
        ensures one: ready()
        ensures two: ready()
        {
            let (; first: local_one, first: local_two) = produce();
            one = local_one;
            two = local_two;
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(duplicate_field))
        .expect_err("a proof-output selector cannot be repeated");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("selector `first` is bound more than once")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    let duplicate_local = duplicate_field.replace(
        "first: local_one, first: local_two",
        "first: local_one, second: local_one",
    );
    let diagnostics = lower_typed_trees(parse_typed_trees(&duplicate_local))
        .expect_err("caller-local evidence names must remain unique");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("term `local_one` is bound more than once")
    }));
}

#[test]
fn proof_output_terms_are_copyable_and_have_no_use_count() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}
        machine produce()
        ensures first: ready()
        ensures second: ready()
        { first = ConcreteEvidence; second = ConcreteEvidence; }
        machine relay()
        ensures relayed_first: ready()
        ensures relayed_second: ready()
        {
            let (; first: local_first, second: local_second) = produce();
            relayed_first = local_first;
            relayed_second = local_first;
        }
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source))
        .expect("one proof term may be copied while another remains unused");
    let invocation = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .next()
        .map(|(_, invocation)| invocation)
        .expect("one checked proof-output call");
    let [first, second] = invocation.outputs.as_slice() else {
        panic!("two checked proof outputs")
    };
    let first = first.output.expect("first output is bound");
    let second = second.output.expect("second output is bound");
    let forwarded = checked
        .facts
        .proof
        .evidence_forwardings
        .iter()
        .filter(|(_, forwarding)| {
            matches!(
                forwarding.source,
                psi_checked_trees::EvidenceAssignmentSource::Forwarded { term }
                    if term == first
            )
        })
        .count();
    assert_eq!(forwarded, 2);
    assert!(
        !checked
            .facts
            .proof
            .evidence_forwardings
            .iter()
            .any(|(_, forwarding)| matches!(
                forwarding.source,
                psi_checked_trees::EvidenceAssignmentSource::Forwarded { term }
                    if term == second
            ))
    );
}

#[test]
fn proof_output_retains_explicit_proposition_discard() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}
        machine produce()
        ensures first: ready()
        ensures second: ready()
        { first = ConcreteEvidence; second = ConcreteEvidence; }
        machine relay()
        ensures relayed: ready()
        {
            let (; first: local, second: _) = produce();
            relayed = local;
        }
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source))
        .expect("copyable proposition evidence may be explicitly discarded");
    let invocation = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .next()
        .map(|(_, invocation)| invocation)
        .expect("one checked proof-output call");
    let [first, second] = invocation.outputs.as_slice() else {
        panic!("two checked proof outputs")
    };
    assert!(first.output.is_some());
    assert_eq!(second.output, None);
}

#[test]
fn proof_output_rejects_a_field_not_published_by_the_callee() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}

        machine produce()
        ensures outgoing: ready()
        {
            outgoing = ConcreteEvidence;
        }

        machine relay()
        ensures relayed: ready()
        {
            let (; invented: local) = produce();
            relayed = local;
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("a proof-output selector cannot be forged");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("publishes no proof-output selector `invented`")
    }));
}

#[test]
fn immediate_proof_output_binds_one_runtime_scalar_call_and_proofs() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}

        machine produce() -> i32
        ensures first: ready()
        ensures second: ready()
        {
            first = ConcreteEvidence;
            second = ConcreteEvidence;
            7
        }

        machine relay() -> i32
        ensures relayed_first: ready()
        ensures relayed_second: ready()
        {
            let (local_value; second: local_second, first: local_first) = produce();
            relayed_first = local_first;
            relayed_second = local_second;
            local_value
        }
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source))
        .expect("the immediate scalar value and complete proof output should check");
    let [typed_invocation] = checked.proof_output_calls.as_slice() else {
        panic!("one typed proof-output invocation expected")
    };
    assert_eq!(typed_invocation.runtime_call_statement_index, Some(0));
    let invocation = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .next()
        .map(|(_, invocation)| invocation)
        .expect("one checked proof-output invocation expected");
    assert_eq!(invocation.outputs.len(), 2);
    let runtime_call = invocation
        .runtime_call
        .expect("the grouped proof metadata retains the ordinary call coordinate");
    assert_eq!(
        (runtime_call.statement_index, runtime_call.call_ordinal),
        (0, 0)
    );
    let relay = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "relay")
        .expect("relay machine");
    let relay_state = &checked.machine_states(relay)[0];
    assert_eq!(relay_state.statement_nodes.count(), 2);
    let contract_call = checked
        .facts
        .proof
        .contract_calls
        .iter()
        .find_map(|(_, call)| {
            (call.caller_machine_symbol == relay.symbol
                && call.statement_index == runtime_call.statement_index
                && call.call_ordinal == runtime_call.call_ordinal)
                .then_some(call)
        })
        .expect("the retained coordinate names one checked contract call");
    assert_eq!(
        contract_call.target_machine_symbol,
        invocation.target_machine_symbol
    );
}

#[test]
fn proof_output_lane_requires_a_runtime_binding_for_a_runtime_result() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}
        machine produce() -> i32
        ensures outgoing: ready()
        { outgoing = ConcreteEvidence; 1 }
        machine relay()
        ensures relayed: ready()
        { let (; outgoing: local) = produce(); relayed = local; }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("a runtime proof-output binding must bind its Type result");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("missing its runtime Type result")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn proof_output_rejects_value_on_unit_and_duplicate_or_discarded_runtime_value() {
    let unit = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}
        machine produce() ensures outgoing: ready() { outgoing = ConcreteEvidence; }
        machine relay()
        ensures relayed: ready()
        { let (runtime; outgoing: local) = produce(); relayed = local; }
    "#;
    let unit = format!("boundary trait MachineControl {{}}\nboundary trait PortIo {{}}\n{unit}");
    let tokens = Lexer::new(&unit).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let diagnostics = lower_symbol_resolved_trees(&resolved)
        .expect_err("a proof-only proof-only call has no value type");
    assert!(
        diagnostics
            .message
            .contains("needs the callee's return type declared")
    );

    let duplicate = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}
        machine produce() -> i32
        ensures first: ready()
        ensures second: ready()
        { first = ConcreteEvidence; second = ConcreteEvidence; 1 }
        machine relay() -> i32
        ensures relayed_first: ready()
        ensures relayed_second: ready()
        {
            let (one; first: local, first: local_two) = produce();
            relayed_first = local;
            relayed_second = local;
            one
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(duplicate))
        .expect_err("a named proof output is unique");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("selector `first` is bound more than once")
    }));

    let discarded = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}
        machine produce() -> i32
        ensures outgoing: ready()
        { outgoing = ConcreteEvidence; 1 }
        machine relay()
        ensures relayed: ready()
        { let (_; outgoing: local) = produce(); relayed = local; }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(discarded))
        .expect_err("runtime Type values are not proposition evidence");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot discard its runtime Type result")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn proof_output_preserves_a_callee_with_runtime_body_work() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}

        machine touch() {}

        machine produce()
        ensures outgoing: ready()
        {
            touch();
            outgoing = ConcreteEvidence;
        }

        machine relay()
        ensures relayed: ready()
        {
            let (; outgoing: local) = produce();
            relayed = local;
        }
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source))
        .expect("a Unit proof-output call must retain its runtime body work");
    let invocation = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .next()
        .map(|(_, invocation)| invocation)
        .expect("one checked proof-output invocation");
    let runtime_call = invocation
        .runtime_call
        .expect("the Unit proof-output invocation retains an ordinary call");
    assert_eq!(
        (runtime_call.statement_index, runtime_call.call_ordinal),
        (0, 0)
    );
    let relay = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "relay")
        .expect("relay machine");
    let [psi_typed_trees::statement::StatementNode::Call(call)] = checked
        .statement_table
        .statements(checked.machine_states(relay)[0].statement_nodes)
    else {
        panic!("the proof-output call must remain in the ordinary runtime stream")
    };
    assert_eq!(call.target.as_str(), "produce");
}

#[test]
fn proof_output_binding_is_not_visible_to_its_own_call() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}

        machine produce()
        requires incoming: ready()
        ensures outgoing: ready()
        {
            outgoing = incoming;
        }

        machine relay()
        ensures relayed: ready()
        {
            let (; outgoing: local) = produce(; local);
            relayed = local;
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("a newly bound proof-output term cannot feed its own invocation");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("cannot require input evidence")
                || diagnostic
                    .message
                    .contains("unknown incoming evidence term `local`")
                || diagnostic.message.contains("proof-only machine")
                || diagnostic
                    .message
                    .contains("proof-only or scalar-result machine")
                || diagnostic.message.contains("zero-argument")
        }),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn proof_output_is_not_visible_before_its_binding() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}

        machine produce()
        ensures outgoing: ready()
        {
            outgoing = ConcreteEvidence;
        }

        machine relay()
        ensures relayed: ready()
        {
            relayed = local;
            let (; outgoing: local) = produce();
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("a future proof output cannot flow backwards");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("local") || diagnostic.message.contains("relayed")
        }),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn proof_output_bound_term_may_remain_unused() {
    let unused = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}
        machine produce() ensures outgoing: ready() { outgoing = ConcreteEvidence; }
        machine relay() { let (; outgoing: local) = produce(); }
    "#;
    lower_typed_trees(parse_typed_trees(unused))
        .expect("a copyable proposition term has no usage-count obligation");
}

#[test]
fn proof_output_runtime_value_cannot_use_proposition_discard() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}
        machine produce() -> i32 ensures outgoing: ready()
        { outgoing = ConcreteEvidence; 7 }
        machine relay() { let (_; outgoing: _) = produce(); }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("the ordinary runtime Type field is not proposition evidence");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot discard its runtime Type result")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn explicit_subjectless_conformance_introduces_named_evidence() {
    let source = r#"
        trait Evidence {
            machine witness(value: i32);
        }
        proposition carries(value: i32) evidence Evidence;

        ConcreteEvidence: satisfies Evidence {
            machine witness(value: i32) {}
        }

        machine produce(value: i32)
        ensures outgoing: carries(value)
        {
            outgoing = ConcreteEvidence;
        }
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source))
        .expect("an explicit complete subjectless conformance should introduce evidence");
    assert_eq!(checked.facts.proof.evidence_forwardings.len(), 1);
    let assignment = checked
        .facts
        .proof
        .evidence_forwardings
        .iter()
        .next()
        .map(|(_, assignment)| assignment)
        .expect("one checked evidence assignment expected");
    let psi_checked_trees::EvidenceAssignmentSource::ProducerConformance {
        conformance,
        evidence_trait,
        rows,
    } = &assignment.source
    else {
        panic!("the assignment should retain its selected producer")
    };
    let selected = checked
        .conformances()
        .iter()
        .find(|candidate| {
            candidate
                .alias
                .as_ref()
                .is_some_and(|alias| alias.as_str() == "ConcreteEvidence")
        })
        .expect("selected conformance remains in checked source facts");
    let evidence = checked
        .traits()
        .iter()
        .find(|candidate| candidate.name.as_str() == "Evidence")
        .expect("evidence trait remains in checked source facts");
    assert_eq!(*conformance, selected.symbol);
    assert_eq!(*evidence_trait, evidence.symbol);
    assert_eq!(rows.len(), 1);
    assert!(rows[0].realization_machine.is_valid());
    assert!(rows[0].realization_state.is_valid());
    assert_eq!(
        checked
            .facts
            .proof
            .evidence_terms
            .get(assignment.output)
            .name,
        "outgoing"
    );
}

#[test]
fn incoming_evidence_binding_shadows_same_named_subjectless_conformance() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}

        machine forward(value: i32)
        requires ConcreteEvidence: carries(value)
        ensures outgoing: carries(value)
        {
            outgoing = ConcreteEvidence;
        }
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source))
        .expect("the lexical incoming evidence binding should shadow the proof-output conformance");
    let [typed_assignment] = checked.evidence_forwardings.as_slice() else {
        panic!("one typed evidence assignment expected")
    };
    assert_eq!(typed_assignment.source_conformance, None);
    let assignment = checked
        .facts
        .proof
        .evidence_forwardings
        .iter()
        .next()
        .map(|(_, assignment)| assignment)
        .expect("one checked evidence assignment expected");
    let psi_checked_trees::EvidenceAssignmentSource::Forwarded { term } = &assignment.source else {
        panic!("the shadowing incoming binding must remain a forwarding source")
    };
    assert_eq!(
        checked.facts.proof.evidence_terms.get(*term).name,
        "ConcreteEvidence"
    );
}

#[test]
fn producer_conformance_must_match_the_declared_evidence_interface() {
    let source = r#"
        trait Evidence {}
        trait DifferentEvidence {}
        proposition carries(value: i32) evidence Evidence;
        WrongProducer: satisfies DifferentEvidence {}

        machine produce(value: i32)
        ensures outgoing: carries(value)
        {
            outgoing = WrongProducer;
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("a producer for a different carrierless interface must reject");
    assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
        "subjectless conformance `WrongProducer` does not provide the exact `Evidence` evidence interface required by `outgoing`"
    )));
}

#[test]
fn instantiated_generic_producer_interface_selects_exact_conformance() {
    let source = r#"
        trait Evidence<T> {
            machine witness(value: T);
        }
        proposition carries<T>(value: T) evidence Evidence<T>;
        ConcreteEvidence: satisfies Evidence<i32> {
            machine witness(value: i32) {}
        }
        data Main { value: i32; }

        machine Main::produce(&self)
        ensures outgoing: carries<i32>(self.value)
        {
            outgoing = ConcreteEvidence;
        }
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source))
        .expect("the concrete proposition argument should instantiate its evidence interface");
    let assignment = checked
        .facts
        .proof
        .evidence_forwardings
        .iter()
        .next()
        .map(|(_, assignment)| assignment)
        .expect("one checked evidence assignment expected");
    let output = checked.facts.proof.evidence_terms.get(assignment.output);
    assert_eq!(output.evidence_type, "Evidence<i32>");
    let interface = output
        .evidence_interface
        .as_ref()
        .expect("the concrete evidence interface should have exact identity");
    assert_eq!(interface.arguments.len(), 1);
    let psi_checked_trees::EvidenceAssignmentSource::ProducerConformance {
        conformance,
        evidence_trait,
        rows,
    } = &assignment.source
    else {
        panic!("the instantiated producer should retain its exact selection")
    };
    assert_eq!(interface.trait_symbol, *evidence_trait);
    assert!(conformance.is_valid());
    assert_eq!(rows.len(), 1);
    let selected = checked
        .conformances()
        .iter()
        .find(|candidate| candidate.symbol == *conformance)
        .expect("exact selected conformance");
    let [typed_assignment] = checked.evidence_forwardings.as_slice() else {
        panic!("one typed evidence assignment expected")
    };
    assert_eq!(typed_assignment.source_conformance, Some(selected.symbol));
    let [selected_argument] = checked
        .type_reference_table
        .type_reference_handles(selected.arguments)
    else {
        panic!("selected conformance should retain one exact argument")
    };
    assert_eq!(
        interface.arguments[0],
        checked
            .normalized_type_identity(*selected_argument)
            .as_str()
    );
}

#[test]
fn instantiated_generic_producer_rejects_wrong_exact_argument() {
    let source = r#"
        trait Evidence<T> {}
        proposition carries<T>(value: T) evidence Evidence<T>;
        WrongEvidence: satisfies Evidence<u32> {}
        data Main { value: i32; }

        machine Main::produce(&self)
        ensures outgoing: carries<i32>(self.value)
        {
            outgoing = WrongEvidence;
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("a producer instantiated at u32 must not inhabit Evidence<i32>");
    assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
        "subjectless conformance `WrongEvidence` does not provide the exact `Evidence<i32>` evidence interface required by `outgoing`"
    )));
}

#[test]
fn unresolved_generic_producer_endpoint_remains_fail_closed() {
    let source = r#"
        trait Evidence<T> {}
        proposition carries<T>(value: T) evidence Evidence<T>;
        ConcreteEvidence: satisfies Evidence<i32> {}

        machine produce<T>(value: T)
        ensures outgoing: carries<T>(value)
        {
            outgoing = ConcreteEvidence;
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("an open generic proposition endpoint cannot select a concrete producer");
    assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
        "subjectless conformance `ConcreteEvidence` cannot provide unresolved generic evidence interface `Evidence<T>` required by `outgoing`"
    )));
}

#[test]
fn unrelated_const_and_machine_binders_do_not_fence_nongeneric_evidence() {
    let source = r#"
        trait Evidence {}
        proposition carries<const N: i32, machine F>(value: i32) evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}
        machine helper() {}
        data Main { value: i32; }

        machine Main::produce(&self)
        ensures outgoing: carries<7, helper>(self.value)
        {
            outgoing = ConcreteEvidence;
        }
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source))
        .expect("unrelated concrete binders must not erase a closed evidence interface");
    let output = checked
        .facts
        .proof
        .evidence_forwardings
        .iter()
        .next()
        .map(|(_, assignment)| checked.facts.proof.evidence_terms.get(assignment.output))
        .expect("one checked evidence assignment expected");
    assert_eq!(output.evidence_type, "Evidence");
    assert!(output.evidence_interface.is_some());
}

#[test]
fn outgoing_producer_does_not_retroactively_discharge_a_call_requirement() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}

        machine consume(value: i32)
        requires carries(value)
        {}

        machine produce(value: i32)
        ensures outgoing: carries(value)
        {
            consume(value);
            outgoing = ConcreteEvidence;
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("an outgoing producer cannot establish an earlier call premise");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot cite `consume`: proposition requirement")
    }));
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
fn named_ensures_rejects_missing_assignment_on_direct_exit() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;
        machine forward(value: i32)
        requires incoming: carries(value)
        ensures outgoing: carries(value)
        {
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("every ordinary exit must assign each named ensures term");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(
                "named ensures evidence `outgoing` is not definitely assigned on the ordinary exit"
            )),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn named_ensures_rejects_repeated_assignment_on_one_path() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;
        machine forward(value: i32)
        requires incoming: carries(value)
        ensures outgoing: carries(value)
        {
            outgoing = incoming;
            outgoing = incoming;
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("one output term cannot be assigned twice on one path");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("named ensures evidence `outgoing` is assigned more than once")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn named_ensures_need_not_be_assigned_on_crash_only_exit() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;
        machine abort(value: i32)
        requires incoming: carries(value)
        ensures outgoing: carries(value)
        crashes Abort
        {
            crash Abort;
        }
    "#;
    lower_typed_trees(parse_typed_trees(source))
        .expect("a crash-only path is not an ordinary proof-output return");
}

#[test]
fn named_ensures_are_definitely_assigned_on_every_named_outcome() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;
        machine forward(value: i32, choose_left: bool)
        requires incoming: carries(value)
        ensures outgoing: carries(value)
        {
            transition choose_left {
                true -> left()
                false -> right()
            }

            state left() {
                outgoing = incoming;
            }

            state right() {
                outgoing = incoming;
            }
        }
    "#;
    lower_typed_trees(parse_typed_trees(source))
        .expect("each named ordinary outcome assigns the output exactly once");
}

#[test]
fn named_ensures_rejects_one_unassigned_named_outcome() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;
        machine forward(value: i32, choose_left: bool)
        requires incoming: carries(value)
        ensures outgoing: carries(value)
        {
            transition choose_left {
                true -> left()
                false -> right()
            }

            state left() {
                outgoing = incoming;
            }

            state right() {
            }
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("the unassigned named outcome must reject");
    assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
        "named ensures evidence `outgoing` is not definitely assigned on the ordinary exit through forward::right"
    )));
}

#[test]
fn named_ensures_assignment_after_terminal_dispatch_does_not_reach_its_arms() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;
        machine forward(value: i32, choose_left: bool)
        requires incoming: carries(value)
        ensures outgoing: carries(value)
        {
            transition choose_left {
                true -> left()
                false -> right()
            }
            outgoing = incoming;

            state left() {}
            state right() {}
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("an erased assignment after terminal dispatch cannot backdate itself");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("named ensures evidence `outgoing` is not definitely assigned")
    }));
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
fn named_transition_evidence_forwards_across_state_arrivals() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;

        machine forward(value: i32)
        requires incoming: carries(value)
        {
            transition { _ -> first(value; incoming) }

            state first(value: i32)
            requires first_evidence: carries(value);
            {
                transition { _ -> second(value; first_evidence) }
            }

            state second(value: i32)
            requires second_evidence: carries(value);
            {
            }
        }
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source))
        .expect("named transition evidence should bind each exact state-arrival lane");
    assert_eq!(checked.facts.proof.contract_evidence_arguments.len(), 2);
    assert!(checked.facts.proof.evidence_terms.iter().any(|(_, term)| {
        term.name == "first_evidence"
            && matches!(
                term.owner,
                psi_checked_trees::ContractProofFactOwner::MachineState { .. }
            )
    }));
}

#[test]
fn named_transition_requires_explicit_evidence_lane() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;

        machine forward(value: i32)
        requires incoming: carries(value)
        {
            transition { _ -> next(value) }

            state next(value: i32)
            requires required: carries(value);
            {
            }
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("ambient state-arrival facts must not synthesize erased transition arguments");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("supplies 0 erased evidence arguments but its named requires lane has 1")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn named_transition_rejects_wrong_evidence_term() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;
        proposition differs(value: i32) evidence Evidence;

        machine forward(value: i32)
        requires incoming: differs(value)
        {
            transition { _ -> next(value; incoming) }

            state next(value: i32)
            requires required: carries(value);
            {
            }
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("a transition evidence term must inhabit the exact target-state proposition");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not inhabit erased requires position 0")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn named_state_requires_rejects_fact_only_evidence_binding() {
    let source = r#"
        proposition ready(value: i32);

        machine forward(value: i32) {
            transition { _ -> next(value) }

            state next(value: i32)
            requires proof: ready(value);
            {
            }
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("a named state arrival requires must carry witness evidence");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(
                "state `next` named requires evidence `proof` binds fact-only proposition `ready`"
            )),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn concrete_trait_named_witness_lanes_bind_inherited_facts_to_satisfier_terms() {
    let source = r#"
        trait Evidence {}
        proposition left(value: i32) evidence Evidence;
        proposition right(value: i32) evidence Evidence;

        trait ForwardContract {
            machine forward(value: i32)
            requires public_left: left(value)
            requires public_right: right(value)
            ensures left_out: left(value)
            ensures right_out: right(value);
        }

        machine forward(item: i32)
        satisfies ForwardContract::forward
        requires local_left: left(item)
        requires local_right: right(item)
        ensures left_out: left(item)
        ensures right_out: right(item)
        {
            left_out = local_left;
            right_out = local_right;
        }
    "#;

    let checked = lower_typed_trees(parse_typed_trees(source))
        .expect("a concrete satisfier may rename inputs while retaining pinned outputs");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .expect("concrete satisfier");
    let state = checked
        .machine_states(machine)
        .first()
        .expect("entry state");
    let inherited = checked
        .facts
        .proof
        .contract_facts
        .iter()
        .filter_map(|(_, fact)| {
            (fact.owner
                == psi_checked_trees::ContractProofFactOwner::MachineState {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                })
            .then_some(fact)
        })
        .collect::<Vec<_>>();
    assert_eq!(inherited.len(), 4);
    let inherited_terms = inherited
        .iter()
        .map(|fact| {
            checked.facts.proof.evidence_terms.get(
                fact.evidence_term
                    .expect("named inherited fact must retain term"),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        inherited_terms
            .iter()
            .map(|term| (term.kind, term.lane_position, term.name.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (
                psi_checked_trees::ContractProofFactKind::Requires,
                0,
                "local_left",
            ),
            (
                psi_checked_trees::ContractProofFactKind::Requires,
                1,
                "local_right",
            ),
            (
                psi_checked_trees::ContractProofFactKind::Ensures,
                0,
                "left_out",
            ),
            (
                psi_checked_trees::ContractProofFactKind::Ensures,
                1,
                "right_out",
            ),
        ]
    );
}

#[test]
fn concrete_trait_named_witness_lane_rejects_order_or_interface_drift() {
    let source = r#"
        trait LeftEvidence {}
        trait RightEvidence {}
        proposition left(value: i32) evidence LeftEvidence;
        proposition right(value: i32) evidence RightEvidence;

        trait ForwardContract {
            machine forward(value: i32)
            requires public_left: left(value)
            requires public_right: right(value);
        }

        machine forward(value: i32)
        satisfies ForwardContract::forward
        requires local_right: right(value)
        requires local_left: left(value)
        {}
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("named lanes cannot reorder proposition/interface identities");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "named requires lane 0 does not retain the requirement's exact proposition and evidence interface",
        )
    }));
}

#[test]
fn concrete_trait_named_witness_lane_rejects_missing_or_renamed_output() {
    let missing = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        trait Contract {
            machine run() ensures selected: ready();
        }
        machine run() satisfies Contract::run {}
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(missing))
        .expect_err("a satisfier cannot omit the requirement's output lane");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("named ensures lane has 0 row(s); the requirement owns at least 1")
    }));

    let renamed = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        trait Contract {
            machine run()
            requires incoming: ready()
            ensures selected: ready();
        }
        machine run()
        satisfies Contract::run
        requires local: ready()
        ensures renamed: ready()
        { renamed = local; }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(renamed))
        .expect_err("a satisfier cannot rename a public output selector");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("renames public selector `selected` to `renamed`")
    }));
}

#[test]
fn concrete_trait_named_witness_output_assignment_remains_exactly_once() {
    let missing = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        trait Contract {
            machine run()
            requires incoming: ready()
            ensures selected: ready();
        }
        machine run()
        satisfies Contract::run
        requires local: ready()
        ensures selected: ready()
        {}
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(missing))
        .expect_err("the inherited public output still needs an assignment");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("named ensures evidence `selected` is not definitely assigned")
    }));

    let duplicate = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        trait Contract {
            machine run()
            requires incoming: ready()
            ensures selected: ready();
        }
        machine run()
        satisfies Contract::run
        requires local: ready()
        ensures selected: ready()
        {
            selected = local;
            selected = local;
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(duplicate))
        .expect_err("the inherited public output cannot be assigned twice");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("named ensures evidence `selected` is assigned more than once")
    }));
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
