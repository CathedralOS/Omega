use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

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
    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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
    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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
    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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
    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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
    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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
    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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

    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect("named transition evidence should bind each exact state-arrival lane");
    assert_eq!(checked.facts.proof.contract_evidence_arguments.len(), 2);
    assert!(checked.facts.proof.evidence_terms.iter().any(|(_, term)| {
        term.name == "first_evidence"
            && matches!(
                term.owner,
                checked_trees::ContractProofFactOwner::MachineState { .. }
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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

    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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
                == checked_trees::ContractProofFactOwner::MachineState {
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
                checked_trees::ContractProofFactKind::Requires,
                0,
                "local_left",
            ),
            (
                checked_trees::ContractProofFactKind::Requires,
                1,
                "local_right",
            ),
            (checked_trees::ContractProofFactKind::Ensures, 0, "left_out",),
            (
                checked_trees::ContractProofFactKind::Ensures,
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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
    let diagnostics = lower_typed_trees(parse_typed_trees(missing), &CheckingRequest::settled())
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
    let diagnostics = lower_typed_trees(parse_typed_trees(renamed), &CheckingRequest::settled())
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
    let diagnostics = lower_typed_trees(parse_typed_trees(missing), &CheckingRequest::settled())
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
    let diagnostics = lower_typed_trees(parse_typed_trees(duplicate), &CheckingRequest::settled())
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("an evidence-lane name must resolve to a caller requires term");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("unknown incoming evidence term `absent`")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}
