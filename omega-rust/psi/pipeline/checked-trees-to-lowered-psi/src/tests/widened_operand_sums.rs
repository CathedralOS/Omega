//! A widened operand carries its source carrier's range: `(a as i64)` from an
//! `i32` lies within the `i32` endpoints. A sum of two widened operands is
//! then bounded by its definition, so subtracting a third widened operand
//! still proves inside `i64`.
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
