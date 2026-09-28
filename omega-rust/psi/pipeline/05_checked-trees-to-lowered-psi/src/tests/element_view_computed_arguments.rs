//! A shared element view passed to a computed call: `let m: i32 =
//! self.finder.find_max(s, 0)` with `s: &[i32]` lends the view to the callee
//! exactly as a `&[u8]` byte view does. The computed-call argument rejoin
//! validates the loan's place identity and observation position, which do not
//! depend on the element the view carries, so the element view takes the same
//! check rather than the record-operand one it has no members for.
use crate::{TerminalMachineSelection, lower_machine};

const FIND_MAX: &str = r#"
    data Finder { values: [i32; 5]; }

    machine Finder::find_max(&self, s: &[i32], current: i32) -> i32 {
        transition s.len > 0 {
            true -> find_max(s[1..], max(current, s[0]))
            false -> (current)
        }
    }

    data Main { finder: Finder; result: i32; }

    machine Main::main(&mut self) {
        self.finder.values[0] = 3;
        self.finder.values[1] = 7;
        self.finder.values[2] = 2;
        self.finder.values[3] = 9;
        self.finder.values[4] = 4;
        let s: &[i32] = self.finder.values.as_slice();
        let m: i32 = self.finder.find_max(s, 0);
        self.result = m;
    }
"#;

#[test]
fn an_element_view_argument_to_a_computed_call_lowers_and_verifies() {
    let checked = crate::front_end::checked_program(FIND_MAX);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("an element view lends to a computed call");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the element view loan verifies");
}

/// A byte literal still spells only a byte view: an element-view formal has no
/// literal payload to rejoin.
#[test]
fn an_element_view_formal_takes_no_literal_payload() {
    let source = r#"
        data Main { result: i32; }
        machine count(s: &[i32]) -> i32 { transition { _ -> (0) } }
        machine Main::main(&mut self) {
            let m: i32 = count("ab");
            self.result = m;
        }
    "#;
    let Ok(checked) = crate::front_end::checked_program_result(source) else {
        return;
    };
    assert!(lower_machine(&checked, TerminalMachineSelection::Name("Main::main")).is_err());
}
