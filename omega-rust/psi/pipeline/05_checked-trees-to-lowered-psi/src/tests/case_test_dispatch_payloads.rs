//! A transition dispatching on an owned case-bearing parameter with case-test
//! arms and a closing `_`. The dispatch consumes the subject: every arm's edge
//! disposes of it, including the `_` edge. An arm whose successor arguments
//! read the tested case's payload binds that payload through a case dispatch
//! the guard realizes, whether the arm reads it or forwards constants.
use crate::{TerminalMachineSelection, lower_machine};

fn verifies(arguments: &str) {
    let source = format!(
        r#"
        data Command {{
            case Run(op: i32, n: i32);
            case Noop;
        }}
        data Main {{ acc: i32 in Wrapping; }}
        machine Main::apply_command(&mut self, cmd: Command) {{
            transition cmd {{
                Command::Run {{ op, n }} -> do_run({arguments})
                Command::Noop -> done()
                _ -> done()
            }}
            state do_run(&mut self, op: i32, n: i32) {{
                self.acc = self.acc + (n as i32 in Wrapping);
            }}
            state done(&mut self) {{}}
        }}
        "#
    );
    let checked = crate::front_end::checked_program(&source);
    let lowered = lower_machine(
        &checked,
        TerminalMachineSelection::Name("Main::apply_command"),
    )
    .unwrap_or_else(|error| panic!("`do_run({arguments})` lowers: {error:?}"));
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap_or_else(|error| panic!("`do_run({arguments})` verifies: {error:?}"));
}

#[test]
fn case_test_arms_consume_their_owned_subject() {
    verifies("1, 2");
}

#[test]
fn a_payload_reading_successor_takes_the_payload_its_case_test_selects() {
    verifies("op, n");
    verifies("n, op");
}
