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
fn a_value_arm_returns_an_earlier_local() {
    verifies(
        r#"
        data Item { id: i32; price: i32; }
        data Main { items: [Item; 2]; }
        machine Main::find_price(&mut self, target: i32) -> i32 {
            transition { _ -> check0(target) }
            state check0(&mut self, target: i32) {
                let p0: i32 = self.items[0].price;
                transition self.items[0].id == target {
                    true -> (p0)
                    _ -> check1(target)
                }
            }
            state check1(&mut self, target: i32) {
                let p1: i32 = self.items[1].price;
                transition self.items[1].id == target {
                    true -> (p1)
                    _ -> (0)
                }
            }
        }
        "#,
        "Main::find_price",
    );
}
