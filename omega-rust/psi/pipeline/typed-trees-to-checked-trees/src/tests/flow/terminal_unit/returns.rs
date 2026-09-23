use super::{CheckedUnitStructuralTypeShape, Multiplicity};
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;

#[test]
fn borrowed_case_getter_retains_ordered_refined_scalar_returns() {
    let checked = checked(
        r#"
        data MemoryAlignment [copy] {
            case Alignment1; case Alignment2; case Alignment4; case Alignment8;
        }
        machine MemoryAlignment::get_size_in_bytes(&self) -> u64 [1..=8] {
            transition self {
                MemoryAlignment::Alignment1 -> (1)
                MemoryAlignment::Alignment2 -> (2)
                MemoryAlignment::Alignment4 -> (4)
                MemoryAlignment::Alignment8 -> (8)
            }
        }
    "#,
    );
    let getter = machine_named(&checked, "get_size_in_bytes");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == getter)
        .expect("borrowed four-case getter retains ordinary ordered scalar completion");
    let control = plan
        .scalar_control
        .as_ref()
        .expect("ordered scalar result owner");
    let checked_trees::CheckedScalarStateTerminator::Guarded { arms, fallback } =
        &control.terminator
    else {
        panic!("ordered guards");
    };
    assert!(
        fallback.is_none(),
        "the final case is not an implicit wildcard"
    );
    let arms = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .guarded_exits
        .span(*arms)
        .unwrap();
    assert_eq!(arms.len(), 4);
    assert!(
        arms.windows(2)
            .all(|pair| pair[0].guard_statement_ordinal + 1 == pair[1].guard_statement_ordinal)
    );
}

#[test]
fn ordered_scalar_guards_retain_explicit_fallback() {
    let checked = checked(
        r#"
        machine choose(input: u64) -> u64 {
            transition input {
                0 -> (1)
                1 -> (2)
                _ -> (3)
            }
        }
    "#,
    );
    let machine = machine_named(&checked, "choose");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == machine)
        .expect("ordinary guarded scalar body");
    let checked_trees::CheckedScalarStateTerminator::Guarded { arms, fallback } =
        &plan.scalar_control.as_ref().unwrap().terminator
    else {
        panic!("ordered guards");
    };
    let arms = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .guarded_exits
        .span(*arms)
        .unwrap();
    assert_eq!(arms.len(), 2);
    assert!(
        matches!(fallback, Some(checked_trees::CheckedScalarBranchDestination::Return { statement_ordinal, is_continuation: false }) if *statement_ordinal == arms[1].guard_statement_ordinal + 1)
    );
}

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

/// A static-requirement-dispatched call names the satisfier's private
/// realization as its `target_symbol`; the specialized caller spelled the
/// public requirement. The guarded plan would otherwise bind the private
/// realization as the callee the caller runs.
#[test]
fn guarded_payloadless_identity_call_rejects_static_requirement_dispatch() {
    let checked = checked(
        r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}
        data Outcome [copy] { case Success; case Failure; }
        trait Producer {
            machine Self::produce(&self) -> Outcome;
        }
        data Token {}
        TokenProducer: Token satisfies Producer {
            machine produce(&self) -> Outcome
            ensures Outcome::Success -> { selected: ready(); }
            { selected = ConcreteEvidence; Outcome::Success }
        }
        machine invoke<Element, Order: Element satisfies Producer>(item: &Element) -> Outcome {
            let saved: Outcome = Order::produce(item);
            transition saved {
                Outcome::Success { } -> saved
                Outcome::Failure { } -> saved
            }
        }
        machine caller(token: &Token) -> Outcome { invoke<Token, TokenProducer>(token) }
        "#,
    );
    let template = machine_named(&checked, "invoke");
    let instance = checked
        .machine_specializations
        .iter()
        .find(|specialization| specialization.template == template)
        .expect("one selected `invoke<Token, TokenProducer>` instance")
        .instance;
    let producer = machine_named(&checked, "produce");
    let dispatched = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == instance)
        .into_iter()
        .flat_map(|machine| checked.machine_states(machine))
        .flat_map(|state| checked.statement_table.statements(state.statement_nodes))
        .filter_map(|statement| match statement {
            typed_trees::statement::StatementNode::LocalData(local) => Some(local.initial_value),
            _ => None,
        })
        .filter_map(
            |expression| match checked.expression_table.expression(expression) {
                typed_trees::expression::ExpressionNode::Call(call) => Some(call),
                _ => None,
            },
        )
        .find(|call| call.static_requirement_dispatch.is_some())
        .expect("the specialized body retains the dispatched requirement call");
    assert!(
        checked
            .machines()
            .iter()
            .any(|machine| machine.symbol == producer
                && checked
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == dispatched.target_symbol)),
        "the rewritten target names the private realization"
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_structural_call_returns
            .payloadless_guarded_for_machine(instance)
            .is_none(),
        "a dispatched requirement call is not an ordinary guarded value-call return"
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

#[test]
fn nested_structural_record_result_keeps_composed_plan() {
    let checked = checked(
        r#"
        data Pair { left: u64; right: u64; }
        data Boxed { pair: Pair; tag: u64; }
        machine choose_boxed(pick_left: bool) -> Boxed {
            transition pick_left {
                true -> left()
                _ -> right()
            }
            state left() -> Boxed { Boxed { pair: Pair { left: 1, right: 0 }, tag: 0 } }
            state right() -> Boxed { Boxed { pair: Pair { left: 2, right: 0 }, tag: 0 } }
        }
    "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "choose_boxed"))
        .expect("a record result with a nested structural carrier stays composed");
    let checked_trees::CheckedControlResultPlan::Structural(result) = &plan.result else {
        panic!("the nested record result stays structural");
    };
    assert!(
        result.type_identity.contains("Boxed"),
        "the result names the authored record, found {}",
        result.type_identity
    );
}

#[test]
fn structural_payload_sum_result_keeps_composed_plan() {
    let checked = checked(
        r#"
        data Inner { value: u64; }
        data Outcome { case Valued(inner: Inner); case Empty; }
        machine choose_outcome(pick_value: bool) -> Outcome {
            transition pick_value {
                true -> first()
                _ -> second()
            }
            state first() -> Outcome { Outcome::Empty }
            state second() -> Outcome { Outcome::Empty }
        }
        machine choose_valued(pick_value: bool) -> Outcome {
            transition pick_value {
                true -> valued()
                _ -> empty()
            }
            state valued() -> Outcome { Outcome::Valued { inner: Inner { value: 3 } } }
            state empty() -> Outcome { Outcome::Empty }
        }
    "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "choose_outcome"))
        .expect("a sum result with a structural payload case stays composed");
    let checked_trees::CheckedControlResultPlan::Structural(result) = &plan.result else {
        panic!("the structural-payload sum result stays structural");
    };
    assert!(
        result.type_identity.contains("Outcome"),
        "the result names the authored sum, found {}",
        result.type_identity
    );
    // A structural case payload establishes as an owned structural value: the
    // composed plan keeps the return as a structural result.
    let valued_plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "choose_valued"))
        .expect("a structural case payload stays composed");
    let checked_trees::CheckedControlResultPlan::Structural(valued_result) = &valued_plan.result
    else {
        panic!("the case-payload result stays structural");
    };
    assert!(
        valued_result.type_identity.contains("Outcome"),
        "the result names the authored sum, found {}",
        valued_result.type_identity
    );
}

#[test]
fn scalar_array_field_record_result_composes_with_zeroed_omission() {
    let checked = checked(
        r#"
        data Framed { bytes: [u8; 4]; tag: u64; }
        machine choose_framed(pick_left: bool) -> Framed {
            transition pick_left {
                true -> left()
                _ -> right()
            }
            state left() -> Framed { Framed { tag: 1 } }
            state right() -> Framed { Framed { tag: 2 } }
        }
    "#,
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_for_machine(machine_named(&checked, "choose_framed"))
            .is_some(),
        "an omitted scalar-array field composes through the zeroed fill"
    );
}

#[test]
fn nested_record_field_record_result_still_declines_composed() {
    let checked = checked(
        r#"
        data Inner { value: u64; }
        data Framed { inner: Inner; tag: u64; }
        machine choose_framed(pick_left: bool) -> Framed {
            transition pick_left {
                true -> left()
                _ -> right()
            }
            state left() -> Framed { Framed { tag: 1 } }
            state right() -> Framed { Framed { tag: 2 } }
        }
    "#,
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_for_machine(machine_named(&checked, "choose_framed"))
            .is_none(),
        "an omitted non-scalar structural field stays outside the composed result signature"
    );
}
