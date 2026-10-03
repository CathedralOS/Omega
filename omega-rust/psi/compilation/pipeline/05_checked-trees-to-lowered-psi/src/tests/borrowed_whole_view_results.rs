//! Borrowed descriptors retain original backing through direct parameter,
//! stored-field and narrowed-range returns. Each path reaches independent
//! Terminal verification; producing a descriptor does not grant a lifetime.
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
fn member_whole_view_results_lower_and_verify() {
    // A receiver-rooted `self.data` projection mints receiver custody and
    // emits a `StructuralLeafCopy` over the `&self` parameter's record
    // storage; the borrowed byte-view leaf resolves through the canonical
    // leaf shape and module verification accepts the copy.
    let checked = crate::front_end::checked_program(
        r#"
            data Views<'r> { data: &'r [u8]; tag: u64; }

            machine Views::get<'r>(&self) -> &'r [u8] {
                self.data
            }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Views::get"))
        .expect("member whole-view result composes and lowers");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("member whole-view result verifies");
}

#[test]
fn borrowed_bounded_storage_supports_subslice_results() {
    // A new descriptor can escape when its original bounded backing belongs
    // to the caller. Bounds and backing provenance are separate obligations.
    let checked = crate::front_end::checked_program(
        r#"
            data Views { tag: u64; }

            machine Views::head<'a>(x: &'a [u8; 64]) -> &'a [u8] {
                x[0..64]
            }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Views::head"))
        .expect("borrowed bounded storage supports descriptor results");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the returned subslice retains caller backing and proven bounds");
}
