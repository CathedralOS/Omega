//! An internal state's `Finite` IEEE parameter carries its domain as an
//! arrival row.
//!
//! The closed scalar qualification catalog holds no predicate-bearing
//! domain, so `value: f32 in Finite` becomes the scalar block invariant
//! `-largest <= value && value <= largest` that every arrival proves. The
//! parameter then joins the catalog as a plain `f32`. Machine entry uses
//! the same predicate as a call requirement.

use super::lower_machine;
use crate::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_psi::OperationKind;

const SOURCE: &str = "
    data Main { level: f32; copy: f32; }
    machine Main::observe(main: &mut Main) {
        main.level = 0.5;
        transition { _ -> accept(main, 0.5) }
        state accept(main: &mut Main, value: f32 in Finite) { main.copy = value; }
    }
    machine Main::main(main: &mut Main) { Main::observe(main); }
";

fn ieee(value: f32) -> OperationKind {
    OperationKind::IeeeFloatConstant {
        value: semantic_vocabulary::IeeeFloatValue::Binary32(value.to_bits()),
    }
}

#[test]
fn a_finite_state_parameter_is_proved_at_every_arrival() {
    let checked = crate::front_end::checked_program(SOURCE);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("a Finite state parameter lowers through its arrival row");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the arrival proves the stored value finite");
    assert_eq!(lowered.semantic_module.scalar_block_invariants.len(), 1);
    let mut broken = lowered.semantic_module.clone();
    let mut changed = 0;
    for operation in broken
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
    {
        if operation.kind == ieee(0.5) {
            operation.kind = ieee(f32::INFINITY);
            changed += 1;
        }
    }
    assert!(changed >= 1);
    assert!(
        terminal_verifier::verify_module(
            &broken,
            &lowered.proof_bundle,
            &AdmissionProfile::default()
        )
        .is_err(),
        "an infinite level cannot arrive at a Finite parameter"
    );
}

#[test]
fn a_finite_unit_entry_parameter_carries_a_call_requirement() {
    let checked = crate::front_end::checked_program(
        "data Main { level: f32; }
         machine Main::keep_level(main: &mut Main, value: f32 in Finite) { main.level = value; }
         machine Main::main(main: &mut Main) { Main::keep_level(main, 0.5); }",
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("a Unit entry carries Finite through its call requirement");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("finite argument proves the Unit call requirement");
    for value in [f32::INFINITY, f32::NEG_INFINITY, f32::NAN] {
        let mut broken = lowered.semantic_module.clone();
        let mut changed = 0;
        for operation in broken
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
        {
            if operation.kind == ieee(0.5) {
                operation.kind = ieee(value);
                changed += 1;
            }
        }
        assert!(changed > 0);
        assert!(
            terminal_verifier::verify_module(
                &broken,
                &lowered.proof_bundle,
                &AdmissionProfile::default()
            )
            .is_err()
        );
    }
}
