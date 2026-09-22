//! A by-value `self` consumer takes the checker's consumption as the claim
//! leaving the caller through the call's receiver argument.

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
