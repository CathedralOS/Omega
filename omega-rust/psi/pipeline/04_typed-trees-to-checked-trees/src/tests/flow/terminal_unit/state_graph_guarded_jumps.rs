//! Ordered guarded chains retain every authored literal arm and the wildcard
//! fallback as one composed tail.
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;

#[test]
fn state_graph_retains_ordered_literal_dispatch_chain() {
    let checked = checked(
        r#"
        data Root { choice: i32; hit: i32; }
        machine Root::run(&mut self) {
            self.choice = 2;
            transition self.choice {
                1 -> one()
                2 -> two()
                _ -> other()
            }
            state one(&mut self) { self.hit = 1; }
            state two(&mut self) { self.hit = 2; }
            state other(&mut self) { self.hit = 3; }
        }
    "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "Root::run"))
        .expect("ordered literal dispatch preserves the complete authored chain");
    assert_eq!(plan.states.len(), 4);
    let checked_trees::CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, fallback } =
        &plan.states[0].terminator
    else {
        panic!("literal dispatch chain is not an ordered guarded jump tail");
    };
    assert_eq!(arms.len(), 2);
    assert_eq!(arms[0].successor.statement_ordinal, 1);
    assert_eq!(arms[1].successor.statement_ordinal, 2);
    assert_eq!(fallback.statement_ordinal, 3);
    assert_eq!(arms[0].successor.target_state, plan.states[1].state);
    assert_eq!(arms[1].successor.target_state, plan.states[2].state);
    assert_eq!(fallback.target_state, plan.states[3].state);
    for arm in arms {
        assert!(matches!(
            &arm.guard,
            checked_trees::CheckedCallScalarArgument::Pure(
                checked_trees::CheckedScalarExpression::Boolean(_)
            )
        ));
    }
}

#[test]
fn state_graph_retains_longer_ordered_dispatch_chain() {
    let checked = checked(
        r#"
        data Root { choice: i32; hit: i32; }
        machine Root::run(&mut self) {
            self.choice = 3;
            transition self.choice {
                1 -> one()
                2 -> two()
                3 -> three()
                _ -> other()
            }
            state one(&mut self) { self.hit = 1; }
            state two(&mut self) { self.hit = 2; }
            state three(&mut self) { self.hit = 3; }
            state other(&mut self) { self.hit = 4; }
        }
    "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "Root::run"))
        .expect("longer ordered dispatch preserves the complete authored chain");
    assert_eq!(plan.states.len(), 5);
    let checked_trees::CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, fallback } =
        &plan.states[0].terminator
    else {
        panic!("longer dispatch chain is not an ordered guarded jump tail");
    };
    assert_eq!(arms.len(), 3);
    for (index, arm) in arms.iter().enumerate() {
        assert_eq!(arm.successor.statement_ordinal as usize, index + 1);
        assert_eq!(arm.successor.target_state, plan.states[index + 1].state);
    }
    assert_eq!(fallback.statement_ordinal, 4);
    assert_eq!(fallback.target_state, plan.states[4].state);
}
