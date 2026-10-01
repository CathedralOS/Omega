//! A hosted entry lends its whole receiver into its states; a state parameter
//! that only ever receives that receiver keeps the invocation's own storage.
use crate::{TerminalMachineSelection, lower_machine};

const RECEIVER_LOAN: &str = "domain [u8; 4]::Utf8(bytes)
    requires valid_utf8(bytes);
    data Main { count: u64 in Wrapping; out: [u8; 4] in Utf8; }
    machine Main::run(&mut self) -> u64 {
        self.count = 3;
        transition { _ -> step(&mut self) }
        state step(main: &mut Main) -> u64 {
            main.count = main.count + 1;
            transition main.count < 6 {
                true -> step(main)
                _ -> finish(main)
            }
        }
        state finish(main: &mut Main) -> u64 {
            transition { _ -> (main.count as u64) }
        }
    }";

#[test]
fn a_forwarded_entry_receiver_keeps_the_invocation_place() {
    // `Main` carries a domain-qualified byte field, so a borrowed block
    // parameter over it has no Terminal form; the forwarded receiver must
    // name the entry's own parameter instead.
    let checked = crate::front_end::checked_program(RECEIVER_LOAN);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("a whole receiver forwarded through states lowers and verifies");
    let machine = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    assert!(
        machine
            .blocks
            .iter()
            .all(|block| block.structural_parameters.is_empty()),
        "no state declares the receiver again as a borrowed block parameter"
    );
}

#[test]
fn a_state_also_reached_with_another_record_is_not_the_receiver() {
    // `finish` receives `other` on one edge, so it cannot alias the entry's
    // receiver: it keeps its own borrowed block parameter, which this plain
    // `Main` admits.
    let checked = crate::front_end::checked_program(
        "data Main { count: u64 in Wrapping; flag: u64; }
        machine Main::run(&mut self, other: &mut Main, pick: bool) -> u64 {
            transition pick {
                true -> finish(&mut self)
                _ -> finish(other)
            }
            state finish(main: &mut Main) -> u64 {
                transition { _ -> (main.count as u64) }
            }
        }",
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("a mixed-source state loan lowers through its block parameter");
    let machine = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    assert!(
        machine
            .blocks
            .iter()
            .any(|block| !block.structural_parameters.is_empty()),
        "a state reached with another record keeps its borrowed block parameter"
    );
}
