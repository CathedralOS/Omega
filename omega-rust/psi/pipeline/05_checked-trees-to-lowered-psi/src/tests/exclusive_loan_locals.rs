//! An exclusive loan local (`let seen: &mut u8 = &mut self.byte;`) names one
//! place for its scope: it plans no operation, each write through the name is
//! the ordinary store of the loaned place, and the state's own terminator is
//! not a use of it. A transition that does name the alias would carry it into
//! a successor this pass does not model, and still declines.
use crate::{TerminalMachineSelection, lower_machine};

#[test]
fn a_write_through_an_exclusive_loan_local_lowers_and_verifies() {
    let checked = crate::front_end::checked_program(
        r#"
            data Cell { raw: u8; }

            machine Cell::clear(&mut self) {
                self.raw = 1;
                let seen: &mut u8 = &mut self.raw;
                seen = 0;
                transition self.raw == 0 {
                    true -> done()
                    _ -> done()
                }

                state done(&mut self) { }
            }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Cell::clear"))
        .expect("an exclusive loan local rejoins its loaned place");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the rejoined store verifies");
}

#[test]
fn a_transition_naming_the_alias_still_declines() {
    // The terminator is exempt only when it cannot carry the alias onward;
    // passing it as a successor argument is exactly the escape this pass
    // does not model.
    let checked = crate::front_end::checked_program(
        r#"
            data Cell { raw: u8; }

            machine Cell::clear(&mut self) {
                self.raw = 1;
                let seen: &mut u8 = &mut self.raw;
                seen = 0;
                transition true {
                    true -> keep(seen)
                    _ -> keep(seen)
                }

                state keep(&mut self, held: &mut u8) { held = 2; }
            }
        "#,
    );
    let error = lower_machine(&checked, TerminalMachineSelection::Name("Cell::clear"))
        .expect_err("an alias carried into a successor has no source replay here");
    assert!(
        matches!(
            error,
            crate::lowering_error::LoweringError::InvalidUnitMachinePlan { .. }
                | crate::lowering_error::LoweringError::Unsupported(_)
        ),
        "unexpected error: {error:?}"
    );
}

#[test]
fn a_write_through_a_structural_loan_local_reaches_the_loaned_field() {
    // The referent may be a record: `r.v = 0` is the ordinary store of
    // `self.inner.v`, with the loan's segments in front of the write's own.
    let checked = crate::front_end::checked_program(
        r#"
            data Inner { v: u32; }
            data Cell { inner: Inner; }

            machine Cell::clear(&mut self) {
                self.inner.v = 1;
                let seen: &mut Inner = &mut self.inner;
                seen.v = 0;
                transition self.inner.v == 0 {
                    true -> done()
                    _ -> done()
                }

                state done(&mut self) { }
            }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Cell::clear"))
        .expect("a structural loan local rejoins its loaned field");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the rejoined nested store verifies");
}

#[test]
fn a_loan_of_a_nested_field_splices_both_paths() {
    // The loan's own segments go in front of the write's: `seen.v = 0` on a
    // loan of `self.outer.inner` stores `self.outer.inner.v`.
    let checked = crate::front_end::checked_program(
        r#"
            data Inner { v: u32; }
            data Outer { inner: Inner; }
            data Cell { outer: Outer; }

            machine Cell::clear(&mut self) {
                self.outer.inner.v = 1;
                let seen: &mut Inner = &mut self.outer.inner;
                seen.v = 0;
                transition self.outer.inner.v == 0 {
                    true -> done()
                    _ -> done()
                }

                state done(&mut self) { }
            }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Cell::clear"))
        .expect("a nested loan splices the loaned path ahead of the write's");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the spliced nested store verifies");
}

#[test]
fn a_loan_of_a_fixed_element_keeps_its_index_segment() {
    // A literal index is a segment like any other: the rejoined store must
    // reach `self.cells[1].v`, not element zero.
    let checked = crate::front_end::checked_program(
        r#"
            data Inner { v: u32; }
            data Cell { cells: [Inner; 2]; }

            machine Cell::clear(&mut self) {
                self.cells[1].v = 1;
                let seen: &mut Inner = &mut self.cells[1];
                seen.v = 0;
                transition self.cells[1].v == 0 {
                    true -> done()
                    _ -> done()
                }

                state done(&mut self) { }
            }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Cell::clear"))
        .expect("a loan of a fixed element rejoins through its index");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the indexed store verifies");
}
