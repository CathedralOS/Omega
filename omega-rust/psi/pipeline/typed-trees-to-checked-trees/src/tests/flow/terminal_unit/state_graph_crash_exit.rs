//! A state whose whole body is one authored `crash` exit plans a `Crash`
//! terminator: scalar machines keep their scalar graph, and structurally
//! owned machines retain the crash state inside the composed route.
use crate::tests::flow::terminal_unit::{checked, machine_named};

#[test]
fn state_graph_admits_scalar_result_with_crash_exit_state() {
    let checked = checked(
        r#"
        data Filter { }
        machine Filter::count(&self) -> u64
        crashes Abort
        {
            crash Abort;
        }
    "#,
    );
    let machine = machine_named(&checked, "Filter::count");
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(machine)
        .expect("scalar result machine with a crash-exit state has a scalar graph");
    assert!(graph.states.iter().any(|state| {
        matches!(
            state.terminator,
            checked_trees::CheckedScalarStateTerminator::Crash {
                statement_ordinal: 0
            }
        )
    }));
}

#[test]
fn state_graph_admits_structural_machine_with_crash_exit_state() {
    let checked = checked(
        r#"
        data Box { items: u64; }
        data Outcome [copy] { case Success; case Failure; }
        machine Box::open(&mut self) -> Outcome
        crashes Abort
        {
            transition self.items > 0 {
                true -> done()
                false -> violated()
            }
            state violated(&mut self) -> Outcome {
                crash Abort;
            }
            state done(&mut self) -> Outcome {
                Outcome::Success
            }
        }
    "#,
    );
    let machine = machine_named(&checked, "Box::open");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "structural machine with a crash-exit state plans: {:?}",
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .omission_for_machine(machine)
            )
        });
    let crash_states = plan
        .states
        .iter()
        .filter(|state| {
            matches!(
                state.terminator,
                checked_trees::CheckedComposedUnitControlTerminatorPlan::Crash {
                    statement_ordinal: 0
                }
            )
        })
        .count();
    assert_eq!(crash_states, 1, "exactly one crash-exit state plans");
}
