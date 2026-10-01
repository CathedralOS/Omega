//! A named jump to a sibling state passes the caller's `self` to the successor
//! while the successor holds every loan the jump's arguments carry. The
//! successor's receiver is judged like a callee's: a `&mut self` successor by
//! its write and read footprint, a `&self` successor as a shared
//! whole-receiver operand. Disjoint fields, copies, shared loans beside
//! reads, and successors without `self` remain usable.

use super::check_program;

fn assert_rejected(source: &str, expected: &str) {
    let diagnostics =
        check_program(source).expect_err("the successor's receiver overlaps a jump loan");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(expected)),
        "{source}: {diagnostics:#?}"
    );
}

fn assert_accepted(source: &str) {
    check_program(source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
}

const COUNT_READ: &str =
    "state `next` receives `self.count` as mutable while its mutable receiver reads `self.count`";
const SHARED_RECEIVER: &str =
    "state `next` receives shared receiver overlapping another argument in the same call";

fn jump_into(argument: &str, successor: &str) -> String {
    format!(
        "data Main {{ count: u64; other: u64; }}
        machine Main::peek(&self) -> u64 {{ (self.count) }}
        machine Main::run(&mut self) -> u64 {{
            transition {{ _ -> next({argument}) }}
            {successor}
        }}"
    )
}

#[test]
fn mutable_successor_rejects_receiver_reads_of_the_lent_field() {
    for successor in [
        // Reading after the argument write observes the aliased store.
        "state next(&mut self, counter: &mut u64) -> u64 {
            counter = 5;
            let seen: u64 = self.count;
            (seen)
        }",
        // Order does not matter: the loan suspends the field for the run.
        "state next(&mut self, counter: &mut u64) -> u64 {
            let seen: u64 = self.count;
            counter = 5;
            (seen)
        }",
        // The loan suspends the field even when nothing writes through it.
        "state next(&mut self, counter: &mut u64) -> u64 {
            (self.count)
        }",
        // A receiver helper's read is the successor's read.
        "state next(&mut self, counter: &mut u64) -> u64 {
            counter = 5;
            (self.peek())
        }",
        // Forwarding the loan to a further state that reads the field.
        "state next(&mut self, counter: &mut u64) -> u64 {
            counter = 5;
            transition { _ -> after(counter) }
        }
        state after(&mut self, counter: &mut u64) -> u64 {
            counter = 6;
            (self.count)
        }",
    ] {
        assert_rejected(&jump_into("&mut self.count", successor), COUNT_READ);
    }
}

#[test]
fn a_jump_from_a_later_state_is_judged_the_same() {
    assert_rejected(
        "data Main { count: u64; other: u64; }
        machine Main::run(&mut self) -> u64 {
            transition { _ -> first() }
            state first(&mut self) -> u64 {
                transition { _ -> next(&mut self.count) }
            }
            state next(&mut self, counter: &mut u64) -> u64 {
                counter = 5;
                (self.count)
            }
        }",
        COUNT_READ,
    );
}

#[test]
fn mutable_successor_rejects_receiver_writes_under_the_jump_loans() {
    assert_rejected(
        &jump_into(
            "&mut self.count",
            "state next(&mut self, counter: &mut u64) -> u64 {
                self.count = 7;
                counter = 5;
                (0)
            }",
        ),
        "state `next` receives `self.count` as mutable while its mutable receiver writes `self.count`",
    );
    assert_rejected(
        &jump_into(
            "&self.count",
            "state next(&mut self, counter: &u64) -> u64 {
                self.count = 9;
                (counter)
            }",
        ),
        "state `next` receives `self.count` as read-only while its mutable receiver writes `self.count`",
    );
}

#[test]
fn a_forwarded_loan_covers_the_next_state_too() {
    assert_rejected(
        &jump_into(
            "&mut self.count",
            "state next(&mut self, counter: &mut u64) -> u64 {
                counter = 5;
                transition { _ -> after(counter) }
            }
            state after(&mut self, counter: &mut u64) -> u64 {
                counter = 6;
                self.count = 7;
                (0)
            }",
        ),
        "state `next` receives `self.count` as mutable while its mutable receiver writes `self.count`",
    );
}

#[test]
fn a_jump_that_stops_forwarding_releases_the_loan() {
    for (argument, successor) in [
        // The loan's name dies at the edge into `done`, which then reads and
        // writes the field through the receiver.
        (
            "&mut self.count",
            "state next(&mut self, counter: &mut u64) -> u64 {
                counter = 5;
                transition { _ -> done() }
            }
            state done(&mut self) -> u64 {
                self.count = self.count + 1;
                (self.count)
            }",
        ),
        // A shared view ends the same way before a later state writes.
        (
            "&self.count",
            "state next(&mut self, counter: &u64) -> u64 {
                let seen: u64 = counter;
                transition { _ -> update(seen) }
            }
            state update(&mut self, seen: u64) -> u64 {
                self.count = seen;
                (self.count)
            }",
        ),
        // A loop forwards the loan to itself and releases it at its exit.
        (
            "&mut self.count",
            "state next(&mut self, counter: &mut u64) -> u64 {
                counter = 5;
                transition self.other > 3 {
                    true -> next(counter)
                    false -> done()
                }
            }
            state done(&mut self) -> u64 { (self.count) }",
        ),
    ] {
        assert_accepted(&jump_into(argument, successor));
    }
}

#[test]
fn shared_successor_rejects_an_exclusive_child_argument() {
    // As for `self.next(&mut self.count)` into a `&self` callee, the shared
    // whole receiver overlaps the exclusive child whatever the body reads.
    for successor in [
        "state next(&self, counter: &mut u64) -> u64 {
            counter = 5;
            (self.count)
        }",
        "state next(&self, counter: &mut u64) -> u64 {
            counter = 5;
            (self.other)
        }",
    ] {
        assert_rejected(&jump_into("&mut self.count", successor), SHARED_RECEIVER);
    }
}

#[test]
fn an_exclusive_local_reference_argument_suspends_its_captured_storage() {
    assert_rejected(
        "data Main { count: u64; other: u64; }
        machine Main::run(&mut self) -> u64 {
            let view: &mut u64 = &mut self.count;
            transition { _ -> next(view) }
            state next(&mut self, counter: &mut u64) -> u64 {
                counter = 5;
                (self.count)
            }
        }",
        COUNT_READ,
    );
    // The same operand rejects at a returning call with a receiver operand.
    assert_rejected(
        "data Main { count: u64; other: u64; }
        machine Main::callee(&mut self, counter: &mut u64) -> u64 {
            counter = 5;
            (self.count)
        }
        machine Main::run(&mut self) -> u64 {
            let view: &mut u64 = &mut self.count;
            let seen: u64 = self.callee(view);
            (seen)
        }",
        "state `callee` receives `self.count` as mutable while its mutable receiver reads `self.count`",
    );
}

#[test]
fn jumps_without_receiver_overlap_accept() {
    for (argument, successor) in [
        // The successor touches only a field disjoint from the lent child.
        (
            "&mut self.count",
            "state next(&mut self, counter: &mut u64) -> u64 {
                counter = 5;
                self.other = 3;
                (self.other)
            }",
        ),
        // A copy ends before the jump.
        (
            "self.count",
            "state next(&mut self, counter: u64) -> u64 {
                self.count = counter;
                (self.count)
            }",
        ),
        // Shared loans coexist with reads through either receiver access.
        (
            "&self.count",
            "state next(&self, counter: &u64) -> u64 {
                let seen: u64 = counter;
                (self.count)
            }",
        ),
        (
            "&self.count",
            "state next(&mut self, counter: &u64) -> u64 {
                self.other = 1;
                (self.count)
            }",
        ),
        // Forwarding the loan to a state that reads a disjoint field.
        (
            "&mut self.count",
            "state next(&mut self, counter: &mut u64) -> u64 {
                counter = 5;
                transition { _ -> after(counter) }
            }
            state after(&mut self, counter: &mut u64) -> u64 {
                counter = 6;
                (self.other)
            }",
        ),
        // A successor without `self` receives no receiver.
        (
            "&mut self.count",
            "state next(counter: &mut u64) -> u64 {
                counter = 5;
                (0)
            }",
        ),
    ] {
        assert_accepted(&jump_into(argument, successor));
    }
}

#[test]
fn local_loans_the_jump_does_not_carry_and_receiver_authority_accept() {
    for source in [
        // A local loan the guard last uses ends at the source state's exit.
        "data Main { count: u64; other: u64; }
        machine Main::run(&mut self) -> u64 {
            let view: &mut u64 = &mut self.count;
            view = 1;
            transition view > 0 { true -> next() false -> next() }
            state next(&self) -> u64 { (self.count) }
        }",
        // A local view of a disjoint field lends only that field.
        "data Main { count: u64; other: u64; }
        machine Main::run(&mut self) -> u64 {
            let view: &mut u64 = &mut self.other;
            transition { _ -> next(view) }
            state next(&mut self, counter: &mut u64) -> u64 {
                counter = 5;
                (self.count)
            }
        }",
        // A `&self` state of a `&mut self` machine hands control back to a
        // `&mut self` sibling.
        "data Main { count: u64; other: u64; }
        machine Main::run(&mut self) -> u64 {
            transition { _ -> look() }
            state look(&self) -> u64 {
                transition { _ -> write() }
            }
            state write(&mut self) -> u64 {
                self.count = 1;
                (0)
            }
        }",
    ] {
        assert_accepted(source);
    }
}
