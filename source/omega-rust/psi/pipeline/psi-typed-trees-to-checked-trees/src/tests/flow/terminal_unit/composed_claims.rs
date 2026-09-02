//! Exact structural-claim custody through composed Unit control.

use super::*;

#[test]
fn composes_one_whole_root_linear_claim_through_both_boundary_leaves() {
    let checked = checked(
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
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let machine = plans
        .composed_for_machine(machine_named(&checked, "enter"))
        .expect("the exclusive leaves should retain one shared linear claim");
    let [entry, when_true, when_false] = machine.states.as_slice() else {
        panic!("linear composed Unit plan retains exactly three states")
    };
    let [entry_claim] = entry.entry_claims.as_slice() else {
        panic!("entry should retain one whole-root claim")
    };
    assert!(entry_claim.path.is_empty());
    assert_eq!(entry_claim.parameter_index, 0);
    assert_eq!(entry.structural_parameters.len(), 1);
    let psi_checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional {
        when_true: true_edge,
        when_false: false_edge,
        ..
    } = &entry.terminator
    else {
        panic!("entry should retain its conditional")
    };
    for edge in [true_edge, false_edge] {
        assert!(matches!(
            edge.transfers.as_slice(),
            [transfer]
                if transfer.source_parameter_index == 0
                    && transfer.target_parameter_index == 0
        ));
        assert!(edge.scalar_arguments.is_empty());
        assert!(edge.trivial_affine_discard_parameter_positions.is_empty());
    }
    for leaf in [when_true, when_false] {
        assert_eq!(leaf.structural_parameters.len(), 1);
        assert!(matches!(
            leaf.entry_claims.as_slice(),
            [claim] if claim.claim_identity
                != psi_language_semantics::PermissionClaimIdentity::Unknown
                && claim.parameter_index == 0
                && claim.path.is_empty()
        ));
        assert!(matches!(
            leaf.operations.as_slice(),
            [CheckedUnitEffectOperationPlan::BoundaryCall {
                structural_arguments,
                completion_receipts,
                ..
            }] if matches!(structural_arguments.as_slice(), [argument]
                if argument.source_parameter_index() == Some(0) && argument.path.is_empty())
                && matches!(completion_receipts.as_slice(), [receipt]
                    if receipt.claim_identity == leaf.entry_claims[0].claim_identity
                        && receipt.argument_index == 0)
        ));
    }
    assert_ne!(when_true.entry_claims[0], when_false.entry_claims[0]);
}
