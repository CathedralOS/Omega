//! A receiver machine returning a primitive scalar whose conditional has one
//! arm returning a value and the other naming a state. The value arm alone
//! evaluates the value checking retained under its `Return` role, so an
//! element read behind the guard is evaluated only where the guard selected
//! it; the named arm keeps its ordinary successor.
use crate::{TerminalMachineSelection, lower_machine};

fn verifies(source: &str, machine: &'static str) {
    let checked = crate::front_end::checked_program(source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name(machine))
        .unwrap_or_else(|error| panic!("{machine} lowers: {error:?}"));
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap_or_else(|error| panic!("{machine} verifies: {error:?}"));
}

#[test]
fn a_value_arm_returns_beside_a_named_arm() {
    verifies(
        r#"
        data Main { table: [i32; 5]; }
        machine Main::read_at(&mut self, idx: i32) -> i32 {
            transition idx == 0 {
                true -> (self.table[0])
                _ -> try1(idx)
            }
            state try1(&mut self, idx: i32) {
                transition idx == 1 {
                    true -> (self.table[1])
                    _ -> (self.table[2])
                }
            }
        }
        "#,
        "Main::read_at",
    );
}

#[test]
fn the_value_arm_may_follow_the_named_arm() {
    verifies(
        r#"
        data Main { table: [i32; 5]; }
        machine Main::read_at(&mut self, idx: i32) -> i32 {
            transition idx != 0 {
                true -> try1(idx)
                _ -> (self.table[0])
            }
            state try1(&mut self, idx: i32) {
                transition { _ -> (self.table[1]) }
            }
        }
        "#,
        "Main::read_at",
    );
}
