//! A shared state parameter that receives its source's whole exclusive loan
//! holds that loan: Terminal edges transfer mutable names, so the departed
//! state's exclusive name cannot stay live beside a shared successor view.
use crate::{TerminalMachineSelection, lower_machine};
use terminal_psi::StructuralAccess;

fn block_parameter_accesses(source: &str) -> Vec<StructuralAccess> {
    let checked = crate::front_end::checked_program(source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("run"))
        .expect("the state loan graph lowers and verifies");
    let machine = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.structural_parameters)
        .map(|parameter| parameter.access)
        .collect()
}

#[test]
fn a_shared_successor_holds_the_forwarded_exclusive_loan() {
    // `check` declares `&Tally`, but its only arrival forwards `fill`'s
    // whole `&mut` loan, so its block parameter receives that mutable name.
    let accesses = block_parameter_accesses(
        "data Tally { first: u64; second: u64; }
        machine run(seed: u64) -> u64 requires seed <= 9 {
            let mut tally: Tally;
            tally.first = seed;
            transition { _ -> fill(&mut tally) }
            state fill(tally: &mut Tally) -> u64 {
                tally.second = 5;
                transition { _ -> check(tally) }
            }
            state check(tally: &Tally) -> u64 {
                transition { _ -> (tally.second) }
            }
        }",
    );
    assert!(
        !accesses.is_empty()
            && accesses
                .iter()
                .all(|access| *access == StructuralAccess::MutableBorrow),
        "every block holds the one exclusive loan: {accesses:?}"
    );
}

#[test]
fn a_shared_successor_also_lent_shared_storage_keeps_its_view() {
    // One arrival lends the entry local shared, so `check` cannot claim the
    // exclusive loan its other arrival forwards.
    let accesses = block_parameter_accesses(
        "data Tally { first: u64; second: u64; }
        machine run(pick: bool) -> u64 {
            let mut tally: Tally;
            tally.first = 4;
            transition pick {
                true -> fill(&mut tally)
                _ -> check(&tally)
            }
            state fill(tally: &mut Tally) -> u64 {
                tally.first = 5;
                transition { _ -> check(tally) }
            }
            state check(tally: &Tally) -> u64 {
                transition { _ -> (tally.first) }
            }
        }",
    );
    assert!(
        accesses.contains(&StructuralAccess::SharedBorrow),
        "the mixed-source successor keeps its authored shared view: {accesses:?}"
    );
}
