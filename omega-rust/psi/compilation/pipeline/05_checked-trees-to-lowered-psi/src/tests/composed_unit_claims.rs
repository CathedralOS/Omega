//! Composed Unit boundary attachments, receiver custody and corruption replay.

use super::{CheckedTrees, LoweringError, lower_machine};
use crate::TerminalMachineSelection;
use checked_trees::{CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitEffectOperationPlan};

/// The operation's call when it is a boundary call with a Unit result.
fn boundary_unit_call(
    operation: &mut CheckedUnitEffectOperationPlan,
) -> Option<&mut CheckedUnitCallPlan> {
    operation
        .call_mut()
        .filter(|call| call.is_boundary() && *call.result() == CheckedUnitCallResult::Unit)
}

fn checked_composed_claim() -> checked_trees::CheckedTrees {
    crate::front_end::checked_program(
        r#"
            pub data Receipt [linear] { value: u64; }
            boundary machine Receipt::settle(self) ensures true;

            data Root {}
            machine Root::enter(flag: bool, receipt: Receipt) {
                transition flag {
                    true -> yes(receipt)
                    _ -> no(receipt)
                }
                state yes(receipt: Receipt) { receipt.settle(); }
                state no(receipt: Receipt) { receipt.settle(); }
            }
        "#,
    )
}

#[test]
fn claim_bearing_composed_unit_rejects_plan_and_fact_corruption() {
    let baseline = checked_composed_claim();
    let rejects = |checked: &CheckedTrees, corruption: &str| {
        assert!(
            matches!(
                lower_machine(checked, TerminalMachineSelection::Name("Root::enter")),
                Err(LoweringError::Unsupported(_))
            ),
            "{corruption}"
        );
    };

    let mut edge = baseline.clone();
    let plan = &mut edge.facts.flow.terminal_unit_effects.composed_machines[0];
    let checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, .. } =
        &mut plan.states[0].terminator
    else {
        unreachable!()
    };
    when_true.transfers[0].source =
        checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index: 1 };
    rejects(&edge, "edge parameter");

    let mut claim = baseline.clone();
    claim.facts.flow.terminal_unit_effects.composed_machines[0].states[1].entry_claims[0]
        .claim_identity = language_semantics::PermissionClaimIdentity::Unknown;
    rejects(&claim, "entry claim");

    let mut receipt = baseline.clone();
    let plan = &mut receipt.facts.flow.terminal_unit_effects.composed_machines[0];
    let entry_claim = plan.states[0].entry_claims[0].claim_identity;
    let Some(call) = boundary_unit_call(&mut plan.states[1].operations[0]) else {
        unreachable!()
    };
    call.claim_transfers_mut()[0].claim_identity = entry_claim;
    rejects(&receipt, "receipt identity");

    let mut duplicate = baseline.clone();
    let Some(call) = boundary_unit_call(
        &mut duplicate.facts.flow.terminal_unit_effects.composed_machines[0].states[1].operations
            [0],
    ) else {
        unreachable!()
    };
    let completion_receipts = call.claim_transfers_mut();
    completion_receipts.push(completion_receipts[0].clone());
    rejects(&duplicate, "duplicate receipt");

    let mut omitted = baseline.clone();
    let Some(call) = boundary_unit_call(
        &mut omitted.facts.flow.terminal_unit_effects.composed_machines[0].states[1].operations[0],
    ) else {
        unreachable!()
    };
    call.claim_transfers_mut().clear();
    rejects(&omitted, "missing receipt");

    let mut facts = baseline.clone();
    let leaf = facts.facts.flow.terminal_unit_effects.composed_machines[0].states[1].state;
    let consumption = facts
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .find_map(|(handle, event)| {
            (event.state_symbol == leaf
                && event.kind == language_semantics::PermissionEventKind::Consume)
                .then_some(handle)
        })
        .expect("leaf consumption fact");
    facts
        .facts
        .flow
        .ownership
        .permissions
        .get_mut(consumption)
        .kind = language_semantics::PermissionEventKind::AffineDrop;
    rejects(&facts, "consumption fact");

    let mut boundary = baseline;
    boundary.facts.flow.terminal_unit_effects.boundary_machines[0].attachment_type_identity = None;
    rejects(&boundary, "boundary attachment");
}
