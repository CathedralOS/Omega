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
