use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

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
