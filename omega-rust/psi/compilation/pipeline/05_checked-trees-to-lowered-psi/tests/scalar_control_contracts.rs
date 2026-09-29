//! An ordered scalar control (a case dispatch whose arms return values)
//! keeps its authored `ensures`: the ordinary machine emission turns it into a
//! normal-return guarantee over the controlled result instead of the tail
//! validation refusing the contract. Squalr's `MemoryAlignment::get_size_in_bytes`
//! is this shape.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_verifier::validate_module;

const SOURCE: &str = r#"
    data Alignment { case One; case Two; case Four; }
    machine Alignment::size(&self) -> u64
        ensures 1 <= result && result <= 4
    {
        transition self {
            Alignment::One -> (1)
            Alignment::Two -> (2)
            Alignment::Four -> (4)
        }
    }
"#;

#[test]
fn ordered_scalar_control_keeps_its_ensures_as_a_return_guarantee() {
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &crate::front_end::checked_program(SOURCE),
        TerminalMachineSelection::Name("Alignment::size"),
    )
    .expect("a contracted value dispatch lowers");
    validate_module(&lowered.semantic_module).expect("the lowered module validates");
    let machine = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    assert_eq!(
        machine.contract.ensures.len(),
        1,
        "the authored ensures stays one normal-return guarantee"
    );
}
