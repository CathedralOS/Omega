use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;
use crate::tests::front_end::typed_program_result;

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

    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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
    let checked_trees::EvidenceAssignmentSource::Forwarded { term } = forwarding.source else {
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

    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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

    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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

    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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
fn cloned_generic_proof_output_call_retains_its_lexical_evidence() {
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
        { copied = incoming; }
        machine relay<Element, Selection: Element satisfies Marker>(value: &Element)
        requires source: ready()
        ensures relayed: ready()
        {
            let (; copied: local) = produce<Element, Selection>(value; source);
            relayed = local;
        }
        machine Root::run(&self)
        requires source: ready()
        ensures relayed: ready()
        {
            let (; relayed: local) = relay<Card, CardMarker>(&self.card; source);
            relayed = local;
        }
    "#;
    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect("cloned proof-output calls retain exact lexical evidence");
    for specialization in &checked.machine_specializations {
        assert!(
            checked
                .typed
                .evidence_forwardings
                .iter()
                .any(|forwarding| forwarding.machine_symbol == specialization.instance)
        );
    }
    let relay = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "relay")
        .expect("authored generic relay");
    let instance = checked
        .machine_specializations
        .iter()
        .find(|specialization| specialization.template == relay.symbol)
        .expect("closed relay receipt")
        .instance;
    assert!(
        checked
            .typed
            .proof_output_calls
            .iter()
            .any(|call| call.machine_symbol == relay.symbol)
    );
    assert!(
        checked
            .typed
            .proof_output_calls
            .iter()
            .any(|call| call.machine_symbol == instance)
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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

    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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

    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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

    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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
    let diagnostics = lower_typed_trees(
        parse_typed_trees(duplicate_field),
        &CheckingRequest::settled(),
    )
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
    let diagnostics = lower_typed_trees(
        parse_typed_trees(&duplicate_local),
        &CheckingRequest::settled(),
    )
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

    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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
                checked_trees::EvidenceAssignmentSource::Forwarded { term }
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
                checked_trees::EvidenceAssignmentSource::Forwarded { term }
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

    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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

    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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
    let diagnostics = typed_program_result(&unit)
        .expect_err("a proof-only proof-only call has no value type")
        .remove(0);
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
    let diagnostics = lower_typed_trees(parse_typed_trees(duplicate), &CheckingRequest::settled())
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
    let diagnostics = lower_typed_trees(parse_typed_trees(discarded), &CheckingRequest::settled())
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

    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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
    let [typed_trees::statement::StatementNode::Call(call)] = checked
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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
    lower_typed_trees(parse_typed_trees(unused), &CheckingRequest::settled())
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
    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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
