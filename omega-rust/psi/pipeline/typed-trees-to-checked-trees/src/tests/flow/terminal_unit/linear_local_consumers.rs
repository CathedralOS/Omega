//! Linear and affine values held in locals compose into the calls that
//! consume them: a by-value `self` consumer takes the checker's consumption
//! as the claim leaving the caller, and a method-spelled receiver bound by a
//! local, a literal, or a call result is the same structural-result argument
//! an explicit spelling names.

use super::CheckedUnitEffectOperationPlan;
use crate::tests::flow::terminal_unit::machine_named;
use crate::tests::front_end::checked_program_result;

const RECEIPT: &str = "data Receipt [linear] { code: i32; }\nmachine Receipt::ack(self) {}\n";

fn plan(source: &str, name: &str) -> Vec<CheckedUnitEffectOperationPlan> {
    let checked = checked_program_result(source).expect("the source checks");
    let machine = machine_named(&checked, name);
    let plans = &checked.facts.flow.terminal_unit_effects;
    plans
        .for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "`{name}` composes its consumer call: {:?}",
                plans.omission_for_machine(machine)
            )
        })
        .operations
        .clone()
}

fn consumer_transfers(operations: &[CheckedUnitEffectOperationPlan]) -> Vec<u32> {
    operations
        .iter()
        .find_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::CallUnit {
                claim_transfers, ..
            } => Some(
                claim_transfers
                    .iter()
                    .map(|transfer| transfer.argument_index)
                    .collect(),
            ),
            _ => None,
        })
        .expect("the consumer is an ordinary Unit call")
}

#[test]
fn linear_parameter_consumed_by_a_by_value_self_callee_transfers_its_claim() {
    // The checker records this consumption rather than a transfer; the claim
    // still leaves through the call's receiver argument.
    let operations = plan(
        &format!("{RECEIPT}machine run(issued: Receipt) -> i32 {{ issued.ack(); 0 }}"),
        "run",
    );
    assert!(matches!(
        operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::CallUnit { .. },
            CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. },
            CheckedUnitEffectOperationPlan::Complete { .. },
        ]
    ));
    assert_eq!(consumer_transfers(&operations), [0]);
}

#[test]
fn linear_parameter_transferred_to_a_linear_formal_keeps_the_ordinary_transfer() {
    let operations = plan(
        &format!(
            "{RECEIPT}machine take(r: Receipt) {{ r.ack(); }}
             machine run(issued: Receipt) -> i32 {{ take(issued); 0 }}"
        ),
        "run",
    );
    assert_eq!(consumer_transfers(&operations), [0]);
}

#[test]
fn locals_bound_from_a_parameter_a_literal_or_a_call_result_feed_the_consumer() {
    // The claim rides the completed structural result into the by-value
    // `self` consumer, so the call names no separate transfer row for it;
    // only a parameter-sourced receiver transfers by row.
    for (label, body, expected_prefix) in [
        (
            "linear parameter moved into a local",
            "machine run(issued: Receipt) -> i32 { let forwarded: Receipt = issued; forwarded.ack(); 0 }",
            "EstablishStructuralValue",
        ),
        (
            "linear parameter moved into a local, explicit receiver spelling",
            "machine run(issued: Receipt) -> i32 { let forwarded: Receipt = issued; Receipt::ack(forwarded); 0 }",
            "EstablishStructuralValue",
        ),
        (
            "linear call result bound by a local",
            "machine issue(r: Receipt) -> Receipt { r }
             machine run(r: Receipt) -> i32 { let returned: Receipt = issue(r); returned.ack(); 0 }",
            "StructuralCall",
        ),
    ] {
        let operations = plan(&format!("{RECEIPT}{body}"), "run");
        let kinds = operations
            .iter()
            .map(|operation| {
                format!("{operation:?}")
                    .split([' ', '{', '('])
                    .next()
                    .unwrap()
                    .to_owned()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            [
                expected_prefix,
                "CallUnit",
                "EstablishScalarLocal",
                "Complete"
            ],
            "{label}"
        );
        assert!(consumer_transfers(&operations).is_empty(), "{label}");
    }
}

#[test]
fn affine_literal_local_consumed_through_a_method_spelled_receiver() {
    let operations = plan(
        "data Token { code: i32; }
         machine Token::ack(self) {}
         machine run() -> i32 { let issued: Token = Token { code: 7 }; issued.ack(); 0 }",
        "run",
    );
    assert!(matches!(
        operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                discard_result_on_return: false,
                ..
            },
            CheckedUnitEffectOperationPlan::CallUnit { .. },
            CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. },
            CheckedUnitEffectOperationPlan::Complete { .. },
        ]
    ));
    assert!(consumer_transfers(&operations).is_empty());
}

#[test]
fn a_linear_local_consumed_twice_never_reaches_a_plan() {
    checked_program_result(&format!(
        "{RECEIPT}machine run(issued: Receipt) -> i32 {{ let forwarded: Receipt = issued; forwarded.ack(); forwarded.ack(); 0 }}"
    ))
    .expect_err("the checker owns the double-consumption rejection");
}

#[test]
fn a_fresh_linear_literal_still_stops_at_its_local_binding() {
    // Terminal has no operation establishing a fresh linear claim, so the
    // literal is not collected as a structural value and the sequence stops
    // at the binding rather than admitting a claim it cannot lower.
    let checked = checked_program_result(&format!(
        "{RECEIPT}machine run() -> i32 {{ let issued: Receipt = Receipt {{ code: 7 }}; issued.ack(); 0 }}"
    ))
    .expect("the source checks");
    let machine = machine_named(&checked, "run");
    let plans = &checked.facts.flow.terminal_unit_effects;
    assert!(plans.for_machine(machine).is_none());
    assert!(matches!(
        plans.omission_for_machine(machine).map(|row| &row.stage),
        Some(
            checked_trees::CheckedUnitPlanOmissionStage::LocalConstruction {
                phase: "statement sequence: local data: structural call binding",
                statement_index: Some(0),
                ..
            }
        )
    ));
}
