use super::*;

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
