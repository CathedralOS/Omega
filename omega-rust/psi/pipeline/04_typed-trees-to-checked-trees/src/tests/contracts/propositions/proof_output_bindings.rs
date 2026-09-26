use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;
use crate::tests::front_end::typed_program_result;

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
