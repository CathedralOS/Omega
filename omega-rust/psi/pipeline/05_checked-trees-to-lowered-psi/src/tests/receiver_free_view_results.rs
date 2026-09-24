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
fn receiver_free_parameter_view_result_verifies() {
    // The receiver's storage never flows to the result — only `x`'s shared
    // loan does — so no receiver custody is required.
    let checked = crate::front_end::checked_program(
        r#"
            data Views<'r> { view: &'r [u8]; tag: u64; }

            machine Views::pick<'r>(&self, x: &'r [u8], y: &'r [u8]) -> &'r [u8] {
                x
            }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Views::pick"))
        .expect("receiver-free parameter view composes and lowers");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("receiver-free parameter view verifies");
}

#[test]
fn record_construction_with_view_member_lowers_and_verifies() {
    // `Views { view: x, tag: 0 }` mints the record's `view` member from the
    // shared-borrowed parameter exactly as a member projection does: the
    // result type, the record value, its field sources, and its emission
    // compose, and the borrowed-view leaf passes the record-field module
    // gate.
    let checked = crate::front_end::checked_program(
        r#"
            data Views<'r> { view: &'r [u8]; tag: u64; }

            machine Views::build<'r>(x: &'r [u8]) -> Views<'r> {
                Views { view: x, tag: 0 }
            }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Views::build"))
        .expect("view-member record construction composes and lowers");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("view-member record construction verifies");
}

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

#[test]
fn exclusive_view_member_construction_keeps_its_decline() {
    // A `&mut` view member is not a shared loan — the exclusive custody
    // family still owns this shape and it keeps its result-type decline.
    let checked = crate::front_end::checked_program(
        r#"
            data Exclusive<'r> { view: &'r mut [u8]; tag: u64; }

            machine Exclusive::build<'r>(x: &'r mut [u8]) -> Exclusive<'r> {
                Exclusive { view: x, tag: 0 }
            }
        "#,
    );
    let error = lower_machine(&checked, TerminalMachineSelection::Name("Exclusive::build"))
        .expect_err("exclusive view members keep their result-type decline");
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
fn generic_record_construction_keeps_its_decline() {
    // A genuinely type-parametric record binds type arguments, not just
    // lifetimes — its fields need instantiation machinery and keep their
    // decline.
    let checked = crate::front_end::checked_program(
        r#"
            data Wrap<T> { cell: T; }

            machine Wrap::pack<T>(x: T) -> Wrap<T> {
                Wrap { cell: x }
            }
        "#,
    );
    let error = lower_machine(&checked, TerminalMachineSelection::Name("Wrap::pack"))
        .expect_err("generic record construction keeps its result-type decline");
    assert!(
        matches!(
            error,
            crate::lowering_error::LoweringError::InvalidUnitMachinePlan { .. }
                | crate::lowering_error::LoweringError::Unsupported(_)
        ),
        "unexpected outcome: {error:?}"
    );
}
