//! A `&`-shaped result that traces only to a parameter needs no receiver
//! custody: `&self` may exist, but the returned loan rides the
//! shared-borrowed parameter's own view. On the construction side a record
//! whose members keep shared views (`&[T]`/`&'a V` fields) mints each view
//! field's source exactly as a member projection does — the record owns its
//! scalars and its view descriptors while each referent's loan stays with
//! the view's owner. `&mut` view members and genuinely generic records keep
//! their declines.
use crate::{TerminalMachineSelection, lower_machine};

#[test]
fn record_with_named_view_member_lowers_and_verifies() {
    // A `&'a V` member keeps its reference shell in the declared field type —
    // `Structural { ref(named(V)) }` — matching the identity the `&`-rooted
    // literal value mints. The record admits the member's affine leaf copy:
    // copying the shared loan's descriptor relocates it under a fresh
    // carrier rather than minting custody, and the result's
    // `reference_sources` roster names the parameter's storage for the
    // referent leaf.
    let checked = crate::front_end::checked_program(
        r#"
            data Inner { code: u64; }
            data NamedViews<'r> { inner: &'r Inner; tag: u64; }

            machine NamedViews::build<'r>(x: &'r Inner) -> NamedViews<'r> {
                NamedViews { inner: x, tag: 0 }
            }
        "#,
    );
    let lowered = lower_machine(
        &checked,
        TerminalMachineSelection::Name("NamedViews::build"),
    )
    .expect("named-view member construction composes and lowers");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("named-view member construction verifies");
    // The roster is custody, not decoration: a result that names no source
    // for its reference leaf is rejected.
    let mut unmapped = lowered.semantic_module.clone();
    let entry = unmapped.entry;
    let machine = unmapped
        .machines
        .iter_mut()
        .find(|machine| machine.id == entry)
        .expect("entry machine");
    let terminal_psi::TerminalMachineResult::Structural(result) = &mut machine.result else {
        panic!("structural result");
    };
    assert!(!result.reference_sources.is_empty());
    result.reference_sources.clear();
    assert!(terminal_verifier::validate_module(&unmapped).is_err());
}

#[test]
fn member_subslice_results_keep_their_frontier_decline() {
    // An authored member subslice establishes a fresh view over the
    // receiver's own storage — the ordered-destination custody family that
    // authored subslices belong to still owns this shape.
    let checked = crate::front_end::checked_program(
        r#"
            data Views { data: [u8; 64]; tag: u64; }

            machine Views::head<'r>(&self) -> &'r [u8] {
                self.data[0..64]
            }
        "#,
    );
    let error = lower_machine(&checked, TerminalMachineSelection::Name("Views::head"))
        .expect_err("authored member subslices keep their ordered-destination decline");
    assert!(
        matches!(
            error,
            crate::lowering_error::LoweringError::InvalidUnitMachinePlan { .. }
                | crate::lowering_error::LoweringError::Unsupported(_)
        ),
        "unexpected outcome: {error:?}"
    );
}
