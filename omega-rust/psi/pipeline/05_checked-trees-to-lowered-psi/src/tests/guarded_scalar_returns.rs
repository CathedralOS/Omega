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
