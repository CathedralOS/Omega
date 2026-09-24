//! Borrowed-leaf record construction: binding a `&`-parameter straight into a
//! record field mints the record result the engine returns. A byte-view leaf
//! copies its whole descriptor; a named shared-borrow leaf relocates the
//! parameter's existing loan under an affine carrier, and the result's
//! `reference_sources` roster names the parameter's storage for the referent
//! leaf — no fresh custody is established.
use crate::{TerminalMachineSelection, lower_machine};

#[test]
fn borrowed_byte_view_record_construction_lowers_and_verifies() {
    let checked = crate::front_end::checked_program(
        r#"
            data Wv<'r> { view: &'r [u8]; }

            machine Wv::build<'a>(x: &'a [u8]) -> Wv<'a> {
                Wv { view: x }
            }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Wv::build"))
        .expect("borrowed byte-view record construction composes and lowers");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("borrowed byte-view record construction verifies");
}

#[test]
fn shared_borrow_record_leaf_construction_lowers_and_verifies() {
    let checked = crate::front_end::checked_program(
        r#"
            data Inner { value: u64; }
            data Nv<'r> { view: &'r Inner; }

            machine Nv::build<'a>(x: &'a Inner) -> Nv<'a> {
                Nv { view: x }
            }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Nv::build"))
        .expect("shared-borrow record-leaf construction composes and lowers");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("shared-borrow record-leaf construction verifies");
}
