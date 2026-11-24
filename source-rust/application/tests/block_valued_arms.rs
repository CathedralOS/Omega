//! A dispatch arm may be a block: `0 -> { let port: u32 = 1; port }` runs
//! its statements in the arm's own scope and gives the arm the block's final
//! value, in a `match` and in a `transition`. The parser used to refuse the
//! `{` ("expected expression"), because no expression could hold statements.
//!
//! `wiki/spec/language/patterns.md`: "a comma separates each arm from the
//! next, including after a block-valued arm"; `expressions.md`: "A block
//! introduces local scope and cleanup edges, not a new machine declaration";
//! `state_contracts.md`: "Disjoint arms or blocks may reuse names."
//!
//! The program puts block arms where an arm value goes: a machine's result,
//! a `let` initializer, a call argument, a nested block, a borrowed local
//! whose loan ends before a later write, a destructured case, a record built
//! inside the block, and a terminal call in a `transition`. It crashes unless
//! every one returns its expected value. Each twin changes one thing and is
//! refused where the change is: a block local named after its block, a block
//! value of another type than its sibling arms, and a write while a block
//! local's loan is still live. The stage handoffs are tested at
//! `pipeline/03_symbol-resolved-trees-to-typed-trees/tests/block_values.rs`,
//! `pipeline/05_prepared-typed-trees-to-lowered-program/tests/block_values.rs`
//! and `pipeline/06_lowered-program-to-checked-program/tests/block_values.rs`.

mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Output, Stdio};

fn fixture(tier: &str, case: &str) -> String {
    let path = support::repository_root()
        .join("source-rust/application/tests/fixtures/patterns")
        .join(tier)
        .join(case)
        .join("main.omg");
    std::fs::read_to_string(path).unwrap()
}

/// Run `omega` on a copy of one source file, so nothing the compiler writes
/// lands in the checkout.
fn invoke(source: &str, arguments: &[&str]) -> (Output, String) {
    let scratch = TemporaryDirectory::create("block-valued-arms", false).unwrap();
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

/// The twin `case` is the run program with `from` replaced by `to`, and
/// `--check` refuses it with each `expected` line: the diagnostic, then
/// where it points.
fn refused(case: &str, from: &str, to: &str, expected: &[&str]) {
    let run = fixture("run", "block_arms");
    assert_eq!(
        run.matches(from).count(),
        1,
        "{case}: the change has one site"
    );
    assert_eq!(run.replace(from, to), fixture("fail", case), "{case}");
    let (output, report) = invoke(&fixture("fail", case), &["--check"]);
    assert!(!output.status.success(), "{report}");
    for expected in expected {
        assert!(report.contains(expected), "{case}: {report}");
    }
}

fn runs_both(source: &str) {
    let (output, report) = invoke(source, &["run", "--both"]);
    assert_eq!(output.status.code(), Some(0), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn block_arms_run_natively_and_in_psi() {
    runs_both(&fixture("run", "block_arms"));
}

#[test]
fn a_block_local_is_not_visible_after_its_block() {
    refused(
        "block_local_after_block",
        "inner + 1 }, _ -> 3 };\n    port\n",
        "inner + 1 }, _ -> 3 };\n    inner\n",
        &[
            "this expression has no visible value declaration in its source scope",
            "main.omg:18:5",
        ],
    );
}

#[test]
fn a_block_value_must_have_the_type_of_its_sibling_arms() {
    refused(
        "block_tail_type",
        "{ let port: u32 = 1; port }, _ -> 2, }",
        "{ let port: u32 = 1; true }, _ -> 2, }",
        &[
            "this value has scalar carrier `bool` but the match's common result type requires `u32`",
            "main.omg:9:44",
        ],
    );
}

#[test]
fn a_write_while_a_block_locals_loan_is_live_is_refused() {
    refused(
        "block_loan_write",
        "{ let view: @Counter = @counter; read(view) }, _ -> 0 };\n    counter.value = 9;\n",
        "{ let view: @Counter = @counter; counter.value = 9; read(view) }, _ -> 0 };\n",
        &[
            "storage is changed while a conflicting reference remains live (conflicting loans)",
            "main.omg:34:80",
        ],
    );
}

#[test]
fn selected_block_writes_survive_the_join() {
    let source = fixture("run", "block_assignment");
    runs_both(&source);

    let immutable = source.replacen("let mutable other:", "let other:", 1);
    let (output, report) = invoke(&immutable, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("writing to an immutable owned binding requires `mutable`")
            && report.contains("main.omg:6:27"),
        "{report}"
    );
}

#[test]
fn a_read_after_distinct_bounded_writes_preserves_their_common_range() {
    let source = fixture("run", "block_storage_bounds");
    runs_both(&source);
    for (old, new, at) in [
        (
            "total: u32 = 4;",
            "total: u32 = 4294967295;",
            "main.omg:6:11",
        ),
        (
            "    total + 1",
            "    total = 4294967295; total + 1",
            "main.omg:6:31",
        ),
        (
            "stock.value = 7;",
            "stock.value = 4294967295;",
            "main.omg:12:7",
        ),
        ("count = 8;", "count = 4294967295;", "main.omg:32:11"),
        (
            "cell.value = 5;",
            "cell.value = 4294967295;",
            "main.omg:44:7",
        ),
    ] {
        assert_eq!(source.matches(old).count(), 1, "{old}");
        let (output, report) = invoke(&source.replace(old, new), &["--check"]);
        assert!(!output.status.success(), "{report}");
        assert!(
            report.contains("may overflow `u32`") && report.contains(at),
            "{report}"
        );
    }

    let moved = source.replace("}, copied)", "}, ticket.slot)");
    let (output, report) = invoke(&moved, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("owned storage is used after its value was transferred or consumed")
            && report.contains("main.omg:23:15"),
        "{report}"
    );
}
