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
fn record_construction_with_view_member_reaches_the_record_gate() {
    // `Views { view: x, tag: 0 }` mints the record's `view` member from the
    // shared-borrowed parameter exactly as a member projection does: the
    // unclaimed chain admits the result type, the record value, its field
    // sources, and its emission — the decline left is the record-field
    // validation arm the Terminal channel gate has not admitted yet.
    let checked = crate::front_end::checked_program(
        r#"
            data Views<'r> { view: &'r [u8]; tag: u64; }

            machine Views::build<'r>(x: &'r [u8]) -> Views<'r> {
                Views { view: x, tag: 0 }
            }
        "#,
    );
    let error = lower_machine(&checked, TerminalMachineSelection::Name("Views::build"))
        .expect_err("view-member record construction stops at the record-field module gate");
    assert!(
        matches!(
            error,
            crate::lowering_error::LoweringError::InvalidTerminalModule(
                terminal_verifier::ModuleError::RecordResultMismatch { .. }
            )
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
