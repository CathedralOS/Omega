//! Authored `<'a>` machine binders erase at checking: the composed plan's
//! type identities and custody paths never spell the lifetime name, so a
//! lifetime-declared machine admits the same graph route as its plain twin.
//! Generic `<T>` binders still need instantiation machinery and stay out.
use crate::{TerminalMachineSelection, lower_machine};

#[test]
fn lifetime_declared_mut_self_machine_lowers_and_verifies() {
    let checked = crate::front_end::checked_program(
        r#"
            data Counter { value: u64; }

            machine Counter::bump<'a>(&mut self) {
                self.value = (self.value as u64 in Wrapping) + (1 as u64 in Wrapping);
            }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Counter::bump"))
        .expect("lifetime-declared machine composes and lowers");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("lifetime-declared machine verifies");
}

#[test]
fn lifetime_declared_view_return_reaches_same_frontier_as_plain_twin() {
    // `&'a [u8]` results admit the same plan route as `&[u8]` results: the
    // shared frontier is the borrowed-view return destination arm, not the
    // binder declaration.
    let checked = crate::front_end::checked_program(
        r#"
            data Store { bytes: [u8; 64]; }

            machine Store::view<'a>(&self, n: u64) -> &'a [u8] {
                transition (n <= self.bytes.len) {
                    true -> (self.bytes[0..n])
                    false -> (self.bytes[0..64])
                }
            }
        "#,
    );
    let error = lower_machine(&checked, TerminalMachineSelection::Name("Store::view"))
        .expect_err("borrowed view results still need the view destination arm");
    assert!(
        matches!(
            error,
            crate::lowering_error::LoweringError::Unsupported(message)
                if message.contains("ordered structural destination")
        ),
        "unexpected decline: {error:?}"
    );
}

#[test]
fn generic_type_parameter_machines_stay_outside_graph_custody() {
    let checked = crate::front_end::checked_program(
        r#"
            data Boxed { value: u64; }

            machine Boxed::get_as<T>(x: T) -> T {
                x
            }
        "#,
    );
    let error = lower_machine(&checked, TerminalMachineSelection::Name("Boxed::get_as"))
        .expect_err("generic type-parameter machines need instantiation machinery");
    assert!(
        matches!(
            error,
            crate::lowering_error::LoweringError::InvalidUnitMachinePlan { .. }
                | crate::lowering_error::LoweringError::Unsupported(_)
        ),
        "unexpected outcome: {error:?}"
    );
}
