//! Stage-level coverage for the exact `GlobalValueNumbering` rule through
//! the public `run_psi_optimization` entrance.

mod common;

use common::value;
use lowered_psi_to_lowered_psi::run_psi_optimization;
use optimization::{PsiOptimization, PsiOptimizationSelections};
use terminal_psi::Terminator;

fn selections() -> PsiOptimizationSelections {
    PsiOptimizationSelections::new([PsiOptimization::GlobalValueNumbering]).unwrap()
}

#[test]
fn ranked_machine_renumbers_outside_the_covered_component() {
    // The `Natural` row covers member block b3: its operations stay exact and
    // the duplicate `v21` survives because member contents still name it on
    // the covered backedge. `v22` is an ordinary duplicate outside the
    // covered component and rewrites to `v20`.
    let lowered = common::ranked_cycle_fixture();
    let optimized =
        run_psi_optimization(lowered.clone(), selections()).expect("renumbering executes");
    let machine = &optimized.lowered().semantic_module.machines[0];

    assert_eq!(
        machine.blocks[0]
            .operations
            .iter()
            .map(|operation| operation.id)
            .collect::<Vec<_>>(),
        vec![common::operation_id(20), common::operation_id(21)],
        "v22 is an uncovered duplicate and leaves; member-used v21 keeps its identity"
    );
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &machine.blocks[0].terminator
    else {
        panic!("the entry keeps its conditional")
    };
    assert_eq!(when_true.arguments, &[value(1), value(20)]);
    assert_eq!(when_false.arguments, &[value(1), value(20)]);

    let member = &machine.blocks[2];
    assert_eq!(
        member
            .parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>(),
        vec![value(10), value(11), value(32)],
        "the covered parameter table stays exact"
    );
    assert_eq!(
        member
            .operations
            .iter()
            .map(|operation| operation.id)
            .collect::<Vec<_>>(),
        vec![
            common::operation_id(40),
            common::operation_id(42),
            common::operation_id(41)
        ],
        "member operations stay exact — even the duplicate constant"
    );
    let Terminator::Conditional { when_true, .. } = &member.terminator else {
        panic!("the member keeps its conditional")
    };
    assert_eq!(when_true.arguments, &[value(41), value(11), value(21)]);
    assert_eq!(
        machine.ranked_scc, lowered.semantic_module.machines[0].ranked_scc,
        "the ranking row is untouched"
    );
}
