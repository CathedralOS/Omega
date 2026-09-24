//! A `collection[0..collection.len]` completion resolves to the carrier
//! parameter itself — or the stored `&` field's referent leaf for member
//! projections — so lowering emits the `EstablishReference` straight through to
//! module validation. Its frontier is the structural-type roster that does not
//! yet admit a shared `&[T]` result reference; partial subslices instead keep
//! their derived-place source and its own provenance gate.
use crate::{TerminalMachineSelection, lower_machine, lowering_error::LoweringError};

#[test]
fn parameter_whole_view_result_reaches_module_validation() {
    let checked = crate::front_end::checked_program(
        r#"
            data Nv { tag: u64; }

            machine Nv::head_all(&self, x: &[u8]) -> &[u8] {
                x[0..x.len]
            }
        "#,
    );
    let error = lower_machine(&checked, TerminalMachineSelection::Name("Nv::head_all"))
        .expect_err("whole parameter views stop at the structural-type roster");
    assert!(
        matches!(
            error,
            LoweringError::InvalidTerminalModule(
                terminal_verifier::ModuleError::InvalidStructuralTypeIdentity(_)
            )
        ),
        "unexpected outcome: {error:?}"
    );
}

#[test]
fn member_whole_view_result_reaches_module_validation() {
    let checked = crate::front_end::checked_program(
        r#"
            data Wv<'r> { view: &'r [u8]; tag: u64; }

            machine Wv::head_all(&self) -> &'r [u8] {
                self.view[0..self.view.len]
            }
        "#,
    );
    let error = lower_machine(&checked, TerminalMachineSelection::Name("Wv::head_all"))
        .expect_err("member whole views stop at the structural-type roster");
    assert!(
        matches!(
            error,
            LoweringError::InvalidTerminalModule(
                terminal_verifier::ModuleError::InvalidStructuralTypeIdentity(_)
            )
        ),
        "unexpected outcome: {error:?}"
    );
}

#[test]
fn derived_subslice_results_keep_their_provenance_decline() {
    // An owned carrier's `[0..len]` — and every `[start..end]` that is not the
    // whole carrier — mints a derived subslice place, which the reference
    // roster does not count as an exact formal referent.
    let checked = crate::front_end::checked_program(
        r#"
            data Nv { tag: u64; }

            machine Nv::head_owned(&self, x: [u8; 64]) -> &[u8] {
                x[0..x.len]
            }
        "#,
    );
    let error = lower_machine(&checked, TerminalMachineSelection::Name("Nv::head_owned"))
        .expect_err("derived subslice places keep their custody decline");
    assert!(
        matches!(
            error,
            LoweringError::InvalidTerminalModule(
                terminal_verifier::ModuleError::InvalidReferenceCustody { .. }
            ) | LoweringError::InvalidUnitMachinePlan { .. }
                | LoweringError::Unsupported(_)
        ),
        "unexpected outcome: {error:?}"
    );
}
