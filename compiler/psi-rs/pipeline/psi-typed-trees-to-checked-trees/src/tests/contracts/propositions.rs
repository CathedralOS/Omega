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
        .expect("the lexical incoming evidence binding should shadow the package conformance");
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
        crashes Abort if true;
        {
            crash Abort;
        }
    "#;
    lower_typed_trees(parse_typed_trees(source))
        .expect("a crash-only path is not an ordinary evidence-package return");
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
