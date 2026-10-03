//! Static calls borrow their explicit operands, regardless of which fields the
//! callee happens to read. Disjoint projections, ended loans and shared reads
//! remain usable without an implicit receiver or a callee-footprint exception.

use super::check_program;

fn assert_rejected(source: &str, expected: &str) {
    let diagnostics = check_program(source).expect_err("a call overlaps a live loan");
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

fn with_caller(body: &str) -> String {
    format!(
        "data Main {{ count: u64; other: u64; }}
        machine Main::peek(main: &Main) -> u64 {{ (main.count) }}
        machine Main::peek_count(main: &mut Main) -> u64 {{ (main.count) }}
        machine Main::peek_other(main: &mut Main) -> u64 {{ (main.other) }}
        machine Main::peek_through(main: &mut Main) -> u64 {{ (Main::peek(main)) }}
        machine Main::peek_with(main: &mut Main, value: u64) -> u64 {{ (main.count) }}
        machine read(value: &u64) -> u64 {{ (value) }}
        machine Main::run(main: &mut Main) -> u64 {{ {body} }}"
    )
}

#[test]
fn whole_argument_rejects_a_live_field_loan_even_when_the_callee_reads_elsewhere() {
    for call in [
        "Main::peek_count(main)",
        "Main::peek_through(main)",
        "Main::peek_with(main, view)",
        "Main::peek_other(main)",
        "Main::peek(main)",
    ] {
        assert_rejected(
            &with_caller(&format!(
                "let view: &mut u64 = &mut main.count;
                let seen: u64 = {call};
                view = 3;
                (seen)"
            )),
            "receives `main` while local borrow `view` is still active",
        );
    }
}

#[test]
fn local_record_arguments_obey_the_same_live_loan_rule() {
    assert_rejected(
        &with_caller(
            "let mut local: Main = Main { count: 1, other: 2 };
            let view: &mut u64 = &mut local.count;
            let seen: u64 = Main::peek_count(&mut local);
            view = 3;
            (seen)",
        ),
        "receives `local` while local borrow `view` is still active",
    );
}

#[test]
fn whole_mutable_argument_rejects_a_live_shared_field_loan() {
    assert_rejected(
        &with_caller(
            "let view: &u64 = &main.count;
            let seen: u64 = Main::peek_count(main);
            let again: u64 = view;
            (again)",
        ),
        "receives `main` while local borrow `view` is still active",
    );
}

#[test]
fn calls_accept_ended_loans_disjoint_projections_and_shared_reads() {
    for body in [
        "let view: &mut u64 = &mut main.count;
        view = 3;
        let seen: u64 = Main::peek_count(main);
        (seen)",
        "let view: &mut u64 = &mut main.other;
        let seen: u64 = read(&main.count);
        view = 3;
        (seen)",
        "let view: &mut u64 = &mut main.count;
        let seen: u64 = read(&main.other);
        view = 3;
        (seen)",
        "let view: &u64 = &main.count;
        let seen: u64 = Main::peek(main);
        let again: u64 = view;
        (again)",
    ] {
        assert_accepted(&with_caller(body));
    }
}

#[test]
fn a_static_write_cannot_bypass_a_live_field_loan() {
    assert_rejected(
        "data Main { count: u64; }
        machine Main::bump(main: &mut Main) -> u64 {
            main.count = 9;
            (main.count)
        }
        machine Main::run(main: &mut Main) -> u64 {
            let view: &mut u64 = &mut main.count;
            let seen: u64 = Main::bump(main);
            view = 3;
            (seen)
        }",
        "receives `main` while local borrow `view` is still active",
    );
}

#[test]
fn successor_receives_explicit_context_after_an_uncarried_loan_ends() {
    assert_accepted(
        "data Main { count: u64; other: u64; }
        machine Main::run(main: &mut Main) -> u64 {
            let view: &mut u64 = &mut main.count;
            view = 3;
            transition view > 0 { true -> next(main) false -> next(main) }
            state next(main: &mut Main) -> u64 { (main.count) }
        }",
    );
}
