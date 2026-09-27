use super::lower_machine;
use crate::TerminalMachineSelection;

#[test]
fn unconditional_scalar_return_cannot_replace_or_omit_an_authored_guard() {
    let checked = crate::front_end::checked_program(
        "data Record [copy] { value: i32; other: i32; }
         machine Record::read(&self, choose: bool) -> i32 {
             transition choose { true -> (self.value) false -> (self.other) }
         }",
    );
    lower_machine(&checked, TerminalMachineSelection::Name("Record::read"))
        .expect("both authored guarded returns");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.scalar_control.is_some())
        .unwrap();
    let (_, state) =
        crate::expression_preparation::source_custody::authored_state(&checked, plan.state)
            .unwrap();
    let statement_count = checked
        .statement_table
        .statements(state.statement_nodes)
        .len();
    // An arm may have a perfectly valid Return-role value. Neither selecting
    // that guard directly nor jumping to its final fallback preserves control.
    for statement_ordinal in [0, u32::try_from(statement_count - 1).unwrap()] {
        let mut changed = checked.clone();
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.scalar_control.is_some())
            .unwrap();
        plan.scalar_control.as_mut().unwrap().terminator =
            checked_trees::CheckedScalarStateTerminator::Return { statement_ordinal };
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("Record::read")).is_err(),
            "forged return at {statement_ordinal} cannot discard the authored guard"
        );
    }
}

#[test]
fn guarded_scalar_return_admits_dead_catch_all_arms_after_an_exhaustive_pair() {
    let checked = crate::front_end::checked_program(
        "data Record [copy] { value: i32; other: i32; }
         machine Record::read(&self, choose: bool) -> i32 {
             transition choose { true -> (self.value) false -> (self.other) _ -> (0) }
         }",
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.scalar_control.is_some())
        .unwrap();
    assert!(matches!(
        plan.scalar_control.as_ref().unwrap().terminator,
        checked_trees::CheckedScalarStateTerminator::Conditional { .. }
    ));
    lower_machine(&checked, TerminalMachineSelection::Name("Record::read"))
        .expect("dead catch-all arm after an exhaustive pair keeps the tail");
}

#[test]
fn guarded_scalar_return_rejects_a_continuation_claim_on_the_exhaustive_tail() {
    let checked = crate::front_end::checked_program(
        "data Record [copy] { value: i32; other: i32; }
         machine Record::read(&self, choose: bool) -> i32 {
             transition choose { true -> (self.value) false -> (self.other) _ -> (0) }
         }",
    );
    let mut changed = checked.clone();
    let plan = changed
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| plan.scalar_control.is_some())
        .unwrap();
    let checked_trees::CheckedScalarStateTerminator::Conditional { when_false, .. } =
        &mut plan.scalar_control.as_mut().unwrap().terminator
    else {
        panic!("expected the conditional tail");
    };
    let checked_trees::CheckedScalarBranchDestination::Return {
        is_continuation, ..
    } = when_false
    else {
        panic!("expected a returning false arm");
    };
    *is_continuation = true;
    assert!(
        lower_machine(&changed, TerminalMachineSelection::Name("Record::read")).is_err(),
        "a continuation claim has no authored continuation to answer"
    );
}
