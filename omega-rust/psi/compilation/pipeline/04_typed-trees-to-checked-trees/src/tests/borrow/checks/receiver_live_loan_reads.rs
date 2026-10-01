//! A whole `&mut` receiver reads what its callee reads while every loan live
//! around the call stays live. A live local exclusive loan suspends the
//! storage it captured until its last use, so the callee may not read that
//! storage through `self` even though no operand names the loan. Loans whose
//! last use precedes the call, disjoint storage, and shared loans remain
//! usable.

use super::check_program;

fn assert_rejected(source: &str, expected: &str) {
    let diagnostics =
        check_program(source).expect_err("the receiver read overlaps a live local exclusive loan");
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
    "whose mutable receiver reads `self.count` while local borrow `view` is still active";

fn with_caller(body: &str) -> String {
    format!(
        "data Main {{ count: u64; other: u64; }}
        machine Main::peek(&self) -> u64 {{ (self.count) }}
        machine Main::peek_count(&mut self) -> u64 {{ (self.count) }}
        machine Main::peek_other(&mut self) -> u64 {{ (self.other) }}
        machine Main::peek_through(&mut self) -> u64 {{ (self.peek()) }}
        machine Main::peek_with(&mut self, value: u64) -> u64 {{ (self.count) }}
        machine Main::run(&mut self) -> u64 {{
            {body}
        }}"
    )
}

#[test]
fn receiver_read_rejects_storage_a_live_exclusive_loan_holds() {
    for body in [
        // The loan is used after the call, so it is live across it.
        "let view: &mut u64 = &mut self.count;
        let seen: u64 = self.peek_count();
        view = 3;
        (seen)",
        // A receiver helper's read is the callee's read.
        "let view: &mut u64 = &mut self.count;
        let seen: u64 = self.peek_through();
        view = 3;
        (seen)",
        // Copying the loan's referent into an argument does not end it.
        "let view: &mut u64 = &mut self.count;
        let seen: u64 = self.peek_with(view);
        view = 3;
        (seen)",
    ] {
        assert_rejected(&with_caller(body), COUNT_READ);
    }
}

#[test]
fn receiver_read_of_a_local_record_rejects_its_live_field_loan() {
    assert_rejected(
        &with_caller(
            "let mut local: Main = Main { count: 1, other: 2 };
            let view: &mut u64 = &mut local.count;
            let seen: u64 = local.peek_count();
            view = 3;
            (seen)",
        ),
        "whose mutable receiver reads `local.count` while local borrow `view` is still active",
    );
}

#[test]
fn a_receiver_write_reports_the_live_loan_once() {
    let source = "data Main { count: u64; other: u64; }
        machine Main::bump(&mut self) -> u64 {
            self.count = 9;
            (self.count)
        }
        machine Main::run(&mut self) -> u64 {
            let view: &mut u64 = &mut self.count;
            let seen: u64 = self.bump();
            view = 3;
            (seen)
        }";
    let diagnostics = check_program(source).expect_err("the receiver write overlaps `view`");
    let conflicts: Vec<_> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.message.contains("local borrow `view`"))
        .collect();
    assert_eq!(conflicts.len(), 1, "{diagnostics:#?}");
    assert!(conflicts[0].message.contains("mutates"), "{diagnostics:#?}");
}

#[test]
fn receiver_reads_beside_ended_disjoint_or_shared_loans_accept() {
    for body in [
        // The loan's last use precedes the call.
        "let view: &mut u64 = &mut self.count;
        view = 3;
        let seen: u64 = self.peek_count();
        (seen)",
        // The loan holds a field the callee never reads.
        "let view: &mut u64 = &mut self.other;
        let seen: u64 = self.peek_count();
        view = 3;
        (seen)",
        "let view: &mut u64 = &mut self.count;
        let seen: u64 = self.peek_other();
        view = 3;
        (seen)",
        // A shared loan freezes only mutation; reads coexist with it.
        "let view: &u64 = &self.count;
        let seen: u64 = self.peek_count();
        let again: u64 = view;
        (again)",
    ] {
        assert_accepted(&with_caller(body));
    }
}

#[test]
fn a_jump_successor_reads_after_source_loans_it_does_not_carry_end() {
    for source in [
        "data Main { count: u64; other: u64; }
        machine Main::run(&mut self) -> u64 {
            let view: &mut u64 = &mut self.count;
            view = 3;
            transition view > 0 { true -> next() false -> next() }
            state next(&mut self) -> u64 { (self.count) }
        }",
        // A view derived through a helper has no replayed closure, yet the
        // successor still runs after the source state's scope exit.
        "data Room { name: u64; }
        data Main { rooms: [Room; 3]; }
        machine Main::run(&mut self) -> u64 {
            let index: u64 = 1;
            let room_slice: &mut [Room] = self.rooms.as_mut_slice();
            transition room_slice[index].name == 4 { true -> next() false -> next() }
            state next(&mut self) -> u64 { (self.rooms[2].name) }
        }",
    ] {
        assert_accepted(source);
    }
}
