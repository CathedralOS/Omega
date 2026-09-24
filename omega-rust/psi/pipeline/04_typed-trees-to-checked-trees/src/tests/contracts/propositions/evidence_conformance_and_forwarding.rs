use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

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

    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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
    let checked_trees::EvidenceAssignmentSource::ProducerConformance {
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

    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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
    let checked_trees::EvidenceAssignmentSource::Forwarded { term } = &assignment.source else {
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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

    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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
    let checked_trees::EvidenceAssignmentSource::ProducerConformance {
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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

    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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
    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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
    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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
    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("forwarding cannot change proposition identity");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("because their proposition identities differ")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}
