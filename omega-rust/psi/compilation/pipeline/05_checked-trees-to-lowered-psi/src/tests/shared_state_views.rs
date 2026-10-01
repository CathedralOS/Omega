//! A state edge may restrict a mutable loan to a shared state view.
//!
//! A named `&mut` parameter passed to a state declaring `&T` keeps its
//! place and narrows its access, exactly as the implicit receiver does for a
//! `&self` state. The shared state reads the place but cannot write it, and
//! Terminal verification accepts the narrowed block parameter.

use super::lower_machine;
use crate::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_psi::StructuralAccess;

const SOURCE: &str = "
    data Main { level: u8; seen: u8 in Wrapping; }
    machine Main::observe(main: &mut Main) {
        main.level = 3;
        transition { _ -> inspect(main) }
        state inspect(main: &Main) {
            transition main.level == 3 {
                true -> keep(main)
                _ -> keep(main)
            }
        }
        state keep(main: &Main) { }
    }
    machine Main::main(main: &mut Main) { Main::observe(main); }
";

#[test]
fn a_named_mutable_loan_narrows_to_a_shared_state_view() {
    let checked = crate::front_end::checked_program(SOURCE);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("a mutable loan narrows to the shared successor state");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the narrowed shared view verifies");
    assert!(
        lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.structural_parameters)
            .any(|parameter| parameter.access == StructuralAccess::SharedBorrow),
        "a successor block declares the shared view"
    );
}
