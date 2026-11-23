//! A const argument written as a call names the same type as the literal it
//! evaluates to: `Buffer<width(7u8)>` is `Buffer<8>` when `width(7u8)` is 8,
//! so a `Buffer<width(7u8)>` can be passed where a `Buffer<8>` is required.
//! The compiler has to run `width` before typing can compare the two, and the
//! comparison has to use the value, not the spelling.
//!
//! The twin differs in one line: `width` adds 2, so the argument is
//! `Buffer<9>`. The compiler must explain the different values at the argument
//! that supplies the wrong type.
//!
//! `wiki/spec/language/generics.md`, "Typed static parameters", and
//! `wiki/spec/language/constants.md`, "Evaluation": "A constant position
//! requests semantic evaluation." Stage 04's half of this handoff is tested in
//! the source compiler's
//! `pipeline/04_typed-trees-to-prepared-typed-trees/tests/prepared_value_inputs.rs`.

mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

fn fixture(tier: &str) -> PathBuf {
    support::repository_root()
        .join("source-rust/application/tests/fixtures/constants")
        .join(tier)
        .join("closed_const_argument")
}

fn read(tier: &str) -> String {
    std::fs::read_to_string(fixture(tier).join("main.omg")).unwrap()
}

/// Run `omega` on a copy of one source file, so nothing the compiler writes
/// lands in the checkout.
fn invoke(source: &str, arguments: &[&str]) -> (Output, String) {
    let scratch = TemporaryDirectory::create("closed-const-argument", false).unwrap();
    std::fs::write(scratch.path().join("main.omg"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(arguments)
        .arg("main.omg")
        .current_dir(scratch.path())
        .stdin(Stdio::null())
        .output()
        .expect("the compiler runs on the fixture");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!("{report}");
    (output, report)
}

fn runs_in_both_engines(source: &str) {
    let (output, report) = invoke(source, &["run", "--both"]);
    assert_eq!(output.status.code(), Some(0), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

fn refuses_at_buffer_argument(source: &str, expected: &str) {
    let (output, report) = invoke(source, &["--check"]);
    assert!(!output.status.success(), "{report}");
    let (line, call) = source
        .lines()
        .enumerate()
        .find(|(_, line)| line.contains("literal(buffer)"))
        .unwrap();
    let column = call.find("literal(buffer)").unwrap() + "literal(".len() + 1;
    let location = format!("main.omg:{}:{column}", line + 1);
    let message = format!("error: {expected} while reading the body of `computed`");
    let mut lines = report.lines();
    assert!(
        lines.any(|line| line.trim() == message)
            && lines
                .next()
                .is_some_and(|line| line.trim_end().ends_with(&location)),
        "{report}"
    );
}

#[test]
fn a_call_argument_and_the_literal_it_evaluates_to_are_one_type() {
    runs_in_both_engines(&read("run"));
}

#[test]
fn a_call_argument_that_evaluates_to_another_number_is_refused() {
    assert_eq!(read("run").replace("n +% 1u8", "n +% 2u8"), read("fail"));
    let expected = std::fs::read_to_string(fixture("fail").join("expected.txt")).unwrap();
    refuses_at_buffer_argument(&read("fail"), expected.trim_end());
}

/// The same handoff for a `bool` parameter: `Flag<enabled()>` is `Flag<true>`
/// only while `enabled` returns `true`.
#[test]
fn a_boolean_call_argument_closes_to_its_value() {
    let flags = |value: &str| {
        read("run")
            .replace(
                "machine width(n: u8) -> u8 crashes none { n +% 1u8 }",
                &format!("machine width() -> bool crashes none {{ {value} }}"),
            )
            .replace("Buffer<const N: u8>", "Buffer<const N: bool>")
            .replace("Buffer<8>", "Buffer<true>")
            .replace("Buffer<width(7u8)>", "Buffer<width()>")
    };
    runs_in_both_engines(&flags("true"));
    refuses_at_buffer_argument(
        &flags("false"),
        "type mismatch: the parameter it is passed for requires `Buffer` with `N = true` but this value supplies false",
    );
}
