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
fn a_jump_releases_one_loan_while_forwarding_an_unrelated_view() {
    assert_accepted(
        "data Main { count: u64; }
        machine Main::run(&mut self, bytes: &[u8]) -> u64 {
            transition { _ -> snapshot(&self.count, bytes) }
            state snapshot(&mut self, count: &u64, bytes: &[u8]) -> u64 {
                let saved: u64 = count;
                transition { _ -> writeback(saved, bytes) }
            }
            state writeback(&mut self, saved: u64, bytes: &[u8]) -> u64 {
                self.count = ((saved as u64 in Wrapping) + (bytes.len as u64 in Wrapping)) as u64;
                (self.count)
            }
        }",
    );
}

#[test]
fn a_cycle_reacquires_and_releases_a_receiver_field_loan() {
    assert_accepted(
        "data Main { count: u64; }
        machine Main::run(&mut self) {
            transition { _ -> snapshot(&mut self.count) }
            state snapshot(&mut self, count: &mut u64) {
                let saved: u64 = count;
                transition { _ -> writeback(saved) }
            }
            state writeback(&mut self, saved: u64) {
                self.count = ((saved as u64 in Wrapping) + 1) as u64;
                transition self.count < 70 {
                    true -> snapshot(&mut self.count)
                    false -> done()
                }
            }
            state done(&mut self) {}
        }",
    );
}

#[test]
fn a_finite_cycle_still_rejects_a_write_while_its_field_is_lent() {
    assert_rejected(
        "data Main { count: u64; }
        machine Main::run(&mut self) {
            transition { _ -> snapshot(&mut self.count) }
            state snapshot(&mut self, count: &mut u64) {
                self.count = 70;
                let saved: u64 = count;
                transition { _ -> again(saved) }
            }
            state again(&mut self, saved: u64) {
                transition saved < 70 {
                    true -> snapshot(&mut self.count)
                    false -> done()
                }
            }
            state done(&mut self) {}
        }",
        "state `snapshot` receives `self.count` as mutable while its mutable receiver writes `self.count`",
    );
}

#[test]
fn a_jump_releases_an_exclusive_loan_before_reading_its_backing() {
    assert_accepted(
        "data Main { count: u64; }
        machine Main::run(&mut self, bytes: &[u8]) -> u64 {
            transition { _ -> update(&mut self.count, bytes) }
            state update(&mut self, count: &mut u64, bytes: &[u8]) -> u64 {
                count = 67;
                transition { _ -> observe(bytes) }
            }
            state observe(&mut self, bytes: &[u8]) -> u64 {
                (((self.count as u64 in Wrapping) + (bytes.len as u64 in Wrapping)) as u64)
            }
        }",
    );
}

#[test]
fn a_jump_retains_a_loan_forwarded_through_a_local_reference_alias() {
    assert_rejected(
        "data Main { count: u64; }
        machine Main::run(&mut self, bytes: &[u8]) -> u64 {
            transition { _ -> first(&self.count, bytes) }
            state first(&mut self, count: &u64, bytes: &[u8]) -> u64 {
                let forwarded: &u64 = count;
                transition { _ -> writeback(forwarded, bytes) }
            }
            state writeback(&mut self, observed: &u64, bytes: &[u8]) -> u64 {
                self.count = bytes.len;
                (observed)
            }
        }",
        "state `first` receives `self.count` as read-only while its mutable receiver writes `self.count`",
    );
}

#[test]
fn a_jump_releases_the_exact_loan_after_same_typed_parameters_reorder() {
    assert_accepted(
        "data Main { count: u64; other: u64; }
        machine Main::run(&mut self) -> u64 {
            transition { _ -> first(&self.count, &self.other) }
            state first(&mut self, count: &u64, other: &u64) -> u64 {
                transition { _ -> swapped(other, count) }
            }
            state swapped(&mut self, count: &u64, other: &u64) -> u64 {
                let saved: u64 = other;
                transition { _ -> writeback(count, saved) }
            }
            state writeback(&mut self, other: &u64, saved: u64) -> u64 {
                self.count = saved;
                (other)
            }
        }",
    );
}

#[test]
fn a_jump_retains_the_exact_loan_after_same_typed_parameters_reorder() {
    assert_rejected(
        "data Main { count: u64; other: u64; }
        machine Main::run(&mut self) -> u64 {
            transition { _ -> first(&self.count, &self.other) }
            state first(&mut self, count: &u64, other: &u64) -> u64 {
                transition { _ -> swapped(other, count) }
            }
            state swapped(&mut self, count: &u64, other: &u64) -> u64 {
                transition { _ -> writeback(other) }
            }
            state writeback(&mut self, observed: &u64) -> u64 {
                self.count = 9;
                (observed)
            }
        }",
        "state `first` receives `self.count` as read-only while its mutable receiver writes `self.count`",
    );
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
fn a_shared_machine_entry_lends_no_state_a_mutable_receiver() {
    const ENTRY_AUTHORITY: &str = "state `next` requires a mutable receiver, but its machine's entry receiver is not writable";
    for source in [
        // The successor writes the entry's shared `self`.
        "data Main { count: u64; other: u64; }
        machine Main::run(&self) -> u64 {
            transition { _ -> next() }
            state next(&mut self) -> u64 {
                self.count = 4;
                (self.count)
            }
        }",
        // Declaring `&mut self` claims the authority even without a write.
        "data Main { count: u64; other: u64; }
        machine Main::run(&self) -> u64 {
            transition { _ -> next() }
            state next(&mut self) -> u64 { (self.count) }
        }",
        // An intermediate `&self` state does not change the entry's authority.
        "data Main { count: u64; other: u64; }
        machine Main::run(&self) -> u64 {
            transition { _ -> look() }
            state look(&self) -> u64 {
                transition { _ -> next() }
            }
            state next(&mut self) -> u64 {
                self.count = 4;
                (0)
            }
        }",
    ] {
        assert_rejected(source, ENTRY_AUTHORITY);
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

// Explicit data parameters retain their own root identity. Supplying the parent
// beside a projected child transfers the suspended parent, not a second usable
// exclusive path to the child. These counterparts retain both state arguments.
fn named_jump_into(argument: &str, successor: &str) -> String {
    format!(
        "data Inner {{ tag: u64; }}
        data Main {{ inner: Inner; other: u64; }}
        machine Main::run(main: &mut Main) -> u64 {{
            transition {{ _ -> next(main, {argument}) }}
            {successor}
        }}"
    )
}

#[test]
fn named_parent_allows_child_writes_and_disjoint_parent_access() {
    assert_accepted(&named_jump_into(
        "&mut main.inner",
        "state next(main: &mut Main, loan: &mut Inner) -> u64 {
            loan.tag = 9;
            main.other = 3;
            let seen: u64 = loan.tag;
            (main.other)
        }",
    ));
    assert_accepted(&named_jump_into(
        "&main.inner",
        "state next(main: &mut Main, loan: &Inner) -> u64 {
            main.other = 3;
            let seen: u64 = loan.tag;
            (main.inner.tag)
        }",
    ));
}

#[test]
fn named_parent_rejects_reads_and_writes_of_its_exclusively_lent_child() {
    // Require the callee-footprint judgment: rejecting the entire handoff as
    // duplicate mutable arguments would also reject the valid counterpart.
    for (body, expected) in [
        (
            "main.inner.tag = 7; let seen: u64 = loan.tag; (seen)",
            "while its mutable receiver writes",
        ),
        (
            "loan.tag = 9; (main.inner.tag)",
            "while its mutable receiver reads",
        ),
        (
            "let seen: u64 = main.inner.tag; loan.tag = 9; (seen)",
            "while its mutable receiver reads",
        ),
    ] {
        assert_rejected(
            &named_jump_into(
                "&mut main.inner",
                &format!("state next(main: &mut Main, loan: &mut Inner) -> u64 {{ {body} }}"),
            ),
            expected,
        );
    }
}

#[test]
fn named_parent_rejects_writes_while_its_shared_child_is_live() {
    assert_rejected(
        &named_jump_into(
            "&main.inner",
            "state next(main: &mut Main, loan: &Inner) -> u64 {
                main.inner.tag = 7;
                (loan.tag)
            }",
        ),
        "while its mutable receiver writes",
    );
}

#[test]
fn named_parent_restores_access_only_after_the_child_stops_forwarding() {
    for (argument, successor) in [
        (
            "&mut main.inner",
            "state next(main: &mut Main, loan: &mut Inner) -> u64 {
                loan.tag = 9;
                transition { _ -> after(loan, main) }
            }
            state after(loan: &mut Inner, main: &mut Main) -> u64 {
                loan.tag = 10;
                transition { _ -> done(main) }
            }
            state done(main: &mut Main) -> u64 {
                main.inner.tag = 11;
                (main.inner.tag)
            }",
        ),
        (
            "&main.inner",
            "state next(main: &mut Main, loan: &Inner) -> u64 {
                let saved: u64 = loan.tag;
                transition { _ -> done(saved, main) }
            }
            state done(saved: u64, main: &mut Main) -> u64 {
                main.inner.tag = saved;
                (main.inner.tag)
            }",
        ),
    ] {
        assert_accepted(&named_jump_into(argument, successor));
    }
}

#[test]
fn named_parent_keeps_child_suspension_across_forwarding_and_local_aliases() {
    assert_rejected(
        &named_jump_into(
            "&mut main.inner",
            "state next(main: &mut Main, loan: &mut Inner) -> u64 {
                loan.tag = 9;
                transition { _ -> after(loan, main) }
            }
            state after(loan: &mut Inner, main: &mut Main) -> u64 {
                loan.tag = 10;
                (main.inner.tag)
            }",
        ),
        "while its mutable receiver reads",
    );
    assert_rejected(
        &named_jump_into(
            "&main.inner",
            "state next(main: &mut Main, loan: &Inner) -> u64 {
                let forwarded: &Inner = loan;
                transition { _ -> after(forwarded, main) }
            }
            state after(loan: &Inner, main: &mut Main) -> u64 {
                main.inner.tag = 10;
                (loan.tag)
            }",
        ),
        "while its mutable receiver writes",
    );
}

#[test]
fn named_parent_can_follow_other_authored_parameters() {
    for (body, expected) in [
        ("loan.tag = 9; main.other = seed; (main.other)", None),
        (
            "loan.tag = 9; (main.inner.tag)",
            Some("while its mutable receiver reads"),
        ),
        (
            "main.inner.tag = seed; (loan.tag)",
            Some("while its mutable receiver writes"),
        ),
    ] {
        let source = format!(
            "data Inner {{ tag: u64; }}
            data Main {{ inner: Inner; other: u64; }}
            machine Main::run(seed: u64, main: &mut Main) -> u64 {{
                transition {{ _ -> next(seed, &mut main.inner, main) }}
                state next(seed: u64, loan: &mut Inner, main: &mut Main) -> u64 {{
                    {body}
                }}
            }}"
        );
        match expected {
            Some(expected) => assert_rejected(&source, expected),
            None => assert_accepted(&source),
        }
    }
}

#[test]
fn named_parent_does_not_merge_independent_same_typed_roots() {
    for supplied_parent in ["left", "right"] {
        let source = format!(
            "data Inner {{ tag: u64; }}
            data Main {{ inner: Inner; }}
            machine Main::run(left: &mut Main, right: &mut Main) -> u64 {{
                transition {{ _ -> next({supplied_parent}, &mut right.inner) }}
                state next(main: &mut Main, loan: &mut Inner) -> u64 {{
                    main.inner.tag = 7;
                    loan.tag = 9;
                    (main.inner.tag)
                }}
            }}"
        );
        if supplied_parent == "left" {
            // A child of right is disjoint from left even though the parent
            // data declarations and field identities are identical.
            assert_accepted(&source);
        } else {
            assert_rejected(&source, "while its mutable receiver writes");
        }
    }
}

#[test]
fn named_parent_releases_the_exact_child_after_same_typed_loans_reorder() {
    for (forwarded, accepts) in [("first", true), ("second", false)] {
        let source = format!(
            "data Main {{ count: u64; other: u64; }}
            machine Main::run(main: &mut Main) -> u64 {{
                transition {{ _ -> next(main, &main.count, &main.other) }}
                state next(main: &mut Main, first: &u64, second: &u64) -> u64 {{
                    transition {{ _ -> swapped(second, main, first) }}
                }}
                state swapped(first: &u64, main: &mut Main, second: &u64) -> u64 {{
                    transition {{ _ -> writeback({forwarded}, main) }}
                }}
                state writeback(observed: &u64, main: &mut Main) -> u64 {{
                    main.count = 7;
                    (observed)
                }}
            }}"
        );
        if accepts {
            assert_accepted(&source);
        } else {
            assert_rejected(&source, "while its mutable receiver writes");
        }
    }
}

#[test]
fn named_parent_rejects_the_board_double_exclusive_access_example() {
    assert_rejected(
        "data Inner { tag: i32; }
        data Main { inner: Inner; seen: i32; }
        machine Main::main(main: &mut Main) {
            main.inner.tag = 1;
            transition true { _ -> next(main, &mut main.inner) }
            state next(main: &mut Main, loan: &mut Inner) {
                main.inner.tag = 7;
                main.seen = loan.tag;
            }
        }",
        "while its mutable receiver writes",
    );
}

#[test]
fn ordinary_static_call_does_not_transport_a_suspended_parent() {
    assert_rejected(
        "data Main { count: u64; other: u64; }
         machine Main::observe(main: &mut Main, child: &u64) -> u64 {
             main.count = 3;
             (child)
         }
         machine Main::run(main: &mut Main) -> u64 {
             let seen: u64 = Main::observe(main, &main.count);
             (seen)
         }",
        "while its mutable receiver writes `main.count`",
    );
}

#[test]
fn named_parent_keeps_local_shared_child_and_disjoint_successor_access() {
    for (body, accepts) in [
        ("main.other = loan.tag; (main.inner.tag)", true),
        ("main.inner.tag = 7; (loan.tag)", false),
    ] {
        let source = format!(
            "data Inner {{ tag: u64; }}
             data Main {{ inner: Inner; other: u64; }}
             machine Main::run(main: &mut Main) -> u64 {{
                 let view: &Inner = &main.inner;
                 transition {{ _ -> next(main, view) }}
                 state next(main: &mut Main, loan: &Inner) -> u64 {{ {body} }}
             }}"
        );
        if accepts {
            assert_accepted(&source);
        } else {
            assert_rejected(&source, "while local borrow `view` is still active");
        }
    }
}

#[test]
fn named_parent_rejects_access_under_a_forwarded_local_exclusive_child() {
    for (child_type, projection, read_body, write_body) in [
        (
            "Inner",
            "main.inner",
            "loan.tag = 9; (main.inner.tag)",
            "main.inner.tag = 7; (loan.tag)",
        ),
        (
            "u64",
            "main.inner.tag",
            "loan = 9; (main.inner.tag)",
            "main.inner.tag = 7; (loan)",
        ),
    ] {
        for (body, expected) in [
            (read_body, "while its mutable receiver reads"),
            (write_body, "while local borrow `view` is still active"),
        ] {
            let source = format!(
                "data Inner {{ tag: u64; }}
                 data Main {{ inner: Inner; other: u64; }}
                 machine Main::run(main: &mut Main) -> u64 {{
                     let view: &mut {child_type} = &mut {projection};
                     transition {{ _ -> next(main, view) }}
                     state next(main: &mut Main, loan: &mut {child_type}) -> u64 {{
                         {body}
                     }}
                 }}"
            );
            assert_rejected(&source, expected);
        }
    }
}

#[test]
fn named_parent_restores_a_local_exclusive_child_after_its_last_forward() {
    for (child_type, projection, write) in [
        ("Inner", "main.inner", "loan.tag = 9;"),
        ("u64", "main.inner.tag", "loan = 9;"),
    ] {
        let source = format!(
            "data Inner {{ tag: u64; }}
             data Main {{ inner: Inner; other: u64; }}
             machine Main::run(main: &mut Main) -> u64 {{
                 let view: &mut {child_type} = &mut {projection};
                 transition {{ _ -> next(main, view) }}
                 state next(main: &mut Main, loan: &mut {child_type}) -> u64 {{
                     {write}
                     main.other = 3;
                     transition {{ _ -> done(main) }}
                 }}
                 state done(main: &mut Main) -> u64 {{
                     main.inner.tag = 10;
                     (main.inner.tag)
                 }}
             }}"
        );
        assert_accepted(&source);
    }
}

#[test]
fn ordinary_static_call_keeps_local_child_exclusivity() {
    assert_rejected(
        "data Main { count: u64; other: u64; }
         machine Main::observe(main: &mut Main, child: &u64) -> u64 {
             main.count = 3;
             (child)
         }
         machine Main::run(main: &mut Main) -> u64 {
             let view: &u64 = &main.count;
             let seen: u64 = Main::observe(main, view);
             (seen)
         }",
        "mutates `main.count` while local borrow `view` is still active",
    );
}
