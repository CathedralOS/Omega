use super::*;

#[test]
fn retains_exact_payloadless_guarded_identity_call() {
    let checked = checked(
        r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}
        data Outcome [copy] { case Success; case Failure; }
        data Root {}

        machine Root::produce() -> Outcome
        ensures Outcome::Success -> { selected: ready(); }
        { selected = ConcreteEvidence; Outcome::Success }

        machine Root::caller() -> Outcome {
            let saved: Outcome = Root::produce();
            transition saved {
                Outcome::Success { ; selected: local } -> saved
                Outcome::Failure { } -> saved
            }
        }
        "#,
    );
    let caller = machine_named(&checked, "caller");
    let producer = machine_named(&checked, "produce");
    let plan = checked
        .facts
        .flow
        .terminal_structural_call_returns
        .payloadless_guarded_for_machine(caller)
        .expect("the exhaustive identity arms should retain one guarded call plan");
    assert_eq!(plan.target_machine, producer);
    assert_eq!(plan.call.statement_index, 0);
    assert_eq!(plan.call.call_ordinal, 0);
    assert_eq!(plan.result.multiplicity, Multiplicity::Unrestricted);
    assert!(plan.result.qualifications.is_empty());
    let selected = plan
        .selected_evidence
        .first()
        .expect("the named guarded row should retain its selected caller term");
    assert_eq!(selected.arm_statement_index, 3);
    assert_eq!(
        checked
            .facts
            .proof
            .evidence_terms
            .get(selected.selected_term)
            .name,
        "local"
    );
}

#[test]
fn guarded_payloadless_identity_call_rejects_foreign_attachment() {
    let checked = checked(
        r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}
        data Outcome [copy] { case Success; case Failure; }
        data Root {}
        data Other {}

        machine Root::produce() -> Outcome
        ensures Outcome::Success -> { selected: ready(); }
        { selected = ConcreteEvidence; Outcome::Success }

        machine Other::caller() -> Outcome {
            let saved: Outcome = Root::produce();
            transition saved {
                Outcome::Success { ; selected: local } -> saved
                Outcome::Failure { } -> saved
            }
        }
        "#,
    );
    let caller = machine_named(&checked, "caller");
    assert!(
        checked
            .facts
            .flow
            .terminal_structural_call_returns
            .payloadless_guarded_for_machine(caller)
            .is_none(),
        "the bounded call carrier must not forge the callee attachment onto a foreign caller"
    );
}

#[test]
fn retains_exact_payloadless_case_return_as_a_separate_checked_plan() {
    let checked = checked(
        r#"
        data Outcome [copy] {
            case Success;
            case Failure;
        }
        data Root {}

        machine Root::choose() -> Outcome {
            Outcome::Success
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_structural_returns;
    let machine = machine_named(&checked, "choose");
    assert!(plans.for_machine(machine).is_none());
    let plan = plans
        .payloadless_case_for_machine(machine)
        .expect("the exact zero-input payload-less case constructor is retained");
    assert_eq!(plan.result.multiplicity, Multiplicity::Unrestricted);
    assert!(plan.result.qualifications.is_empty());
    assert_eq!(plan.returned_case_identity, "Success");

    let result_shape = plans
        .structural_types
        .iter()
        .find(|shape| shape.identity == plan.result.type_identity)
        .expect("the plan retains its exact result shape");
    let CheckedUnitStructuralTypeShape::Sum { cases } = &result_shape.shape else {
        panic!("the result must remain a sum")
    };
    assert_eq!(
        cases
            .iter()
            .map(|case| (case.identity.as_str(), case.fields.len()))
            .collect::<Vec<_>>(),
        [("Success", 0), ("Failure", 0)]
    );
}

#[test]
fn retains_guarded_only_payloadless_case_return_but_not_ordinary_contracts() {
    let checked = checked(
        r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}
        data Outcome [copy] { case Success; case Failure; }
        data Root {}

        machine Root::guarded() -> Outcome
        ensures Outcome::Success -> { selected: ready(); true; }
        { selected = ConcreteEvidence; Outcome::Success }

        machine Root::ordinary() -> Outcome
        ensures true;
        { Outcome::Success }
        "#,
    );
    let plans = &checked.facts.flow.terminal_structural_returns;
    assert!(
        plans
            .payloadless_case_for_machine(machine_named(&checked, "guarded"))
            .is_some(),
        "guarded-only contracts preserve the exact payloadless producer plan"
    );
    assert!(
        plans
            .payloadless_case_for_machine(machine_named(&checked, "ordinary"))
            .is_none(),
        "unconditional contracts remain outside this bounded producer rung"
    );
}

#[test]
fn payloadless_case_return_plan_fences_wider_result_and_body_shapes() {
    let checked = checked(
        r#"
        data Outcome [copy] { case Success; case Failure; }
        data Singleton [copy] { case Only; }
        data Payload [copy] { case Empty; case Value(code: u8); }
        data LinearOutcome [linear] { case Success; case Failure; }
        data Root {}

        machine Root::with_parameter(value: u8) -> Outcome { Outcome::Success }
        machine Root::singleton() -> Singleton { Singleton::Only }
        machine Root::payload() -> Payload { Payload::Empty }
        machine Root::linear() -> LinearOutcome { LinearOutcome::Success }
        machine Root::with_contract() -> Outcome
        ensures
            true;
        { Outcome::Success }
        machine Root::with_local() -> Outcome {
            let staged: Outcome = Outcome::Failure;
            Outcome::Success
        }
        machine Root::helper() -> Outcome { Outcome::Failure }
        machine Root::through_call() -> Outcome { Root::helper() }
        machine Root::with_reach() -> Outcome
        reaches PortIo
        { Outcome::Success }
        "#,
    );

    let plans = &checked.facts.flow.terminal_structural_returns;
    for name in [
        "with_parameter",
        "singleton",
        "payload",
        "linear",
        "with_contract",
        "with_local",
        "through_call",
        "with_reach",
    ] {
        assert!(
            plans
                .payloadless_case_for_machine(machine_named(&checked, name))
                .is_none(),
            "{name} must remain outside the narrow payload-less case return rung"
        );
    }
    assert!(
        plans
            .payloadless_case_for_machine(machine_named(&checked, "helper"))
            .is_some(),
        "the fence fixture keeps one independent exact constructor canary"
    );
}
