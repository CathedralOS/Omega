//! A widened operand carries its source carrier's range: `(a as i64)` from an
//! `i32` lies within the `i32` endpoints. A sum or difference of widened
//! operands is then bounded by its definition, so a chain of them keeps
//! proving inside `i64`.
use crate::{TerminalMachineSelection, lower_machine};

fn lowers(source: &str) -> Result<(), String> {
    let checked =
        crate::front_end::checked_program_result(source).map_err(|error| format!("{error:?}"))?;
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::f"))
        .map_err(|error| format!("{error:?}"))?;
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .map(|_| ())
    .map_err(|error| format!("{error:?}"))
}

#[test]
fn a_widened_operand_subtracts_from_a_widened_sum() {
    lowers(
        "data Main {}
        machine Main::f(&mut self, a: i32, b: i32, c: i32) -> i32 {
            let total: i64 = (a as i64) + (b as i64) - (c as i64);
            transition total == 70 { true -> (1) _ -> (0) }
        }",
    )
    .unwrap();
}

#[test]
fn a_difference_of_widened_operands_bounds_the_next_subtraction() {
    lowers(
        "data Main {}
        machine Main::f(&mut self, a: i32, b: i32, c: i32) -> i32 {
            let total: i64 = (a as i64) + (b as i64) - (c as i64) - 1;
            transition total == 70 { true -> (1) _ -> (0) }
        }",
    )
    .unwrap();
}

#[test]
fn operands_already_at_the_wide_carrier_bound_nothing() {
    assert!(
        lowers(
            "data Main {}
            machine Main::f(&mut self, a: i64, b: i64, c: i64) -> i32 {
                let total: i64 = a + b - c;
                transition total == 70 { true -> (1) _ -> (0) }
            }",
        )
        .is_err()
    );
}
