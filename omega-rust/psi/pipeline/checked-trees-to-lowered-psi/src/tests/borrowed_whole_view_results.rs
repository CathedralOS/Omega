//! `&'a [u8]`/`&'a [T]` whole-view results forward the borrowed parameter's own
//! storage as the result: no reference is established and nothing owned is
//! consumed, so the `Parameter` result source passes straight through. Member
//! projections and authored subslices still mint their own custody and keep
//! their declines.
use crate::{TerminalMachineSelection, lower_machine};

#[test]
fn detached_shared_borrowed_byte_view_lowers_and_verifies() {
    let checked = crate::front_end::checked_program(
        r#"
            data Views { tag: u64; }

            machine Views::pick<'a, 'b>(x: &'a [u8], y: &'b [u8]) -> &'a [u8] {
                x
            }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Views::pick"))
        .expect("borrowed whole byte view composes and lowers");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("borrowed whole byte view verifies");
}

#[test]
fn detached_shared_borrowed_element_view_lowers_and_verifies() {
    let checked = crate::front_end::checked_program(
        r#"
            data Item { head: u64; tail: u64; }
            data Views { tag: u64; }

            machine Views::pick_item<'a, 'b>(x: &'a [Item], y: &'b [Item]) -> &'a [Item] {
                x
            }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Views::pick_item"))
        .expect("borrowed whole element view composes and lowers");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("borrowed whole element view verifies");
}

#[test]
fn member_whole_view_results_keep_their_decline() {
    // `&'a self` receivers mint no structural parameter custody, so a whole
    // view field has no ingress loan to name.
    let checked = crate::front_end::checked_program(
        r#"
            data Views<'r> { data: &'r [u8]; tag: u64; }

            machine Views::get<'r>(&self) -> &'r [u8] {
                self.data
            }
        "#,
    );
    let error = lower_machine(&checked, TerminalMachineSelection::Name("Views::get"))
        .expect_err("member whole-view results have no receiver custody yet");
    assert!(
        matches!(
            error,
            crate::lowering_error::LoweringError::InvalidUnitMachinePlan { .. }
                | crate::lowering_error::LoweringError::Unsupported(_)
        ),
        "unexpected outcome: {error:?}"
    );
}

#[test]
fn subslice_view_results_keep_their_frontier_decline() {
    // An authored subslice establishes a new borrowed view over the carrier's
    // storage — its emission still needs the ordered-destination custody that
    // whole-parameter forwards do not.
    let checked = crate::front_end::checked_program(
        r#"
            data Views { tag: u64; }

            machine Views::head<'a>(x: &'a [u8; 64]) -> &'a [u8] {
                x[0..64]
            }
        "#,
    );
    let error = lower_machine(&checked, TerminalMachineSelection::Name("Views::head"))
        .expect_err("authored subslice results keep their ordered-destination decline");
    assert!(
        matches!(
            error,
            crate::lowering_error::LoweringError::InvalidUnitMachinePlan { .. }
                | crate::lowering_error::LoweringError::Unsupported(_)
        ),
        "unexpected outcome: {error:?}"
    );
}
