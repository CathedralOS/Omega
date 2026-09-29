//! Linear and affine values held in locals compose into the calls that
//! consume them: a by-value `self` consumer takes the checker's consumption
//! as the claim leaving the caller, and a method-spelled receiver bound by a
//! local, a literal, or a call result is the same structural-result argument
//! an explicit spelling names.

use super::CheckedUnitEffectOperationPlan;
use crate::tests::flow::terminal_unit::machine_named;
use crate::tests::front_end::checked_program_result;
use checked_trees::CheckedUnitCallResult;

// `ack` stays a checked machine so its callers compose an ordinary Unit call
// edge; the linear receiver discharges into the `settle` contract, which owns
// the boundary crossing.
const RECEIPT: &str = "data Receipt [linear] { code: i32; }\nboundary requirement Receipt::settle(self);\nmachine Receipt::ack(self) { Receipt::settle(self); }\n";

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
            CheckedUnitEffectOperationPlan::Call(call)
                if !call.is_boundary() && *call.result() == CheckedUnitCallResult::Unit =>
            {
                Some(
                    call.claim_transfers()
                        .iter()
                        .map(|transfer| transfer.argument_index)
                        .collect(),
                )
            }
            _ => None,
        })
        .expect("the consumer is an ordinary Unit call")
}

/// The operation's variant name; a call is named by its route and result so
/// an ordinary Unit call stays distinct from every other call shape.
fn operation_kind(operation: &CheckedUnitEffectOperationPlan) -> String {
    if let Some(call) = operation.call() {
        let route = if call.is_boundary() {
            "boundary"
        } else {
            "ordinary"
        };
        let result = match call.result() {
            CheckedUnitCallResult::Unit => "Unit",
            CheckedUnitCallResult::Scalar(_) => "scalar",
            CheckedUnitCallResult::Structural { .. } => "structural",
        };
        return format!("{route} {result} call");
    }
    format!("{operation:?}")
        .split([' ', '{', '('])
        .next()
        .unwrap()
        .to_owned()
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
            "ordinary structural call",
        ),
    ] {
        let operations = plan(&format!("{RECEIPT}{body}"), "run");
        let kinds = operations.iter().map(operation_kind).collect::<Vec<_>>();
        assert_eq!(
            kinds,
            [
                expected_prefix,
                "ordinary Unit call",
                "EstablishScalarLocal",
                "Complete"
            ],
            "{label}"
        );
        assert!(consumer_transfers(&operations).is_empty(), "{label}");
    }
}
