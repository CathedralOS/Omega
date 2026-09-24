//! A state with stores owes every call it makes an owning operation, but a
//! transition that makes no call owes none: `pop_val` binds a call result,
//! stores, then returns the local through a call-free value transition.
use crate::{TerminalMachineSelection, lower_machine};

#[test]
fn a_call_free_value_transition_after_stores_owes_no_call() {
    let source = r#"
        data Vm { sp: i32 in Wrapping; top_value: i32 in Wrapping; }
        data Main { vm: Vm; }
        machine Main::top(&mut self) -> i32 in Wrapping { self.vm.top_value }
        machine Main::pop_val(&mut self) -> i32 {
            let v: i32 in Wrapping = self.top();
            self.vm.sp = self.vm.sp - 1;
            transition {
                _ -> (v as i32)
            }
        }
    "#;
    let checked = crate::front_end::checked_program(source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::pop_val"))
        .unwrap_or_else(|error| panic!("pop_val lowers: {error:?}"));
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap_or_else(|error| panic!("pop_val verifies: {error:?}"));
}
