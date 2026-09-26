//! Two conditional frontiers and three exact effect leaves.
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;

fn conditional_successors(
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
) -> (
    &checked_trees::CheckedStructuralControlSuccessorPlan,
    &checked_trees::CheckedStructuralControlSuccessorPlan,
) {
    let checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional {
        when_true,
        when_false,
        ..
    } = &state.terminator
    else {
        panic!("control state remains conditional")
    };
    (when_true, when_false)
}

#[test]
fn composes_a_reversed_boolean_label_pair_without_an_authored_fallback() {
    let checked = checked(
        r#"
        boundary trait Host { machine exit(code: i32); }
        data Root { flag: bool; }
        machine Root::enter(&mut self) reaches Host {
            self.flag = true;
            transition {
                self.flag == false -> good()
                self.flag == true -> bad()
            }
            state good(&mut self) { Host::exit(70); }
            state bad(&mut self) { Host::exit(71); }
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "enter"))
        .expect("`x == false` / `x == true` over one subject is an exact false fallback");
    let [entry, good, bad] = plan.states.as_slice() else {
        panic!("the reversed Boolean pair keeps the entry conditional and both leaves")
    };
    let (when_true, when_false) = conditional_successors(entry);
    assert_eq!(when_true.target_state, good.state);
    assert_eq!(when_false.target_state, bad.state);
}
