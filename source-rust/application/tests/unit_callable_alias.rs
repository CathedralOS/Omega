//! A callable alias without `->`, such as `data Check = machine(value: u32)
//! crashes Trap;`, can be the type of a static callable parameter:
//! `machine apply<const F: Check>(value: u32) crashes Trap { F(value) }`.
//!
//! The program passes `expect_one`, which crashes unless it receives 1, and
//! calls it through `apply` with 1, so it exits 0 only when the selected
//! machine ran with the argument it was given. Its twin differs in one name:
//! it passes `echo`, which returns a `u32`, and is refused because `Check`
//! returns nothing.
//!
//! The compiler used to refuse the alias itself ("runtime callable has no
//! complete structural type identity"): spelling the callable's identity read
//! its result type, and a signature without `->` has none. It is now spelled
//! as Unit, the same way for the alias and for the machine selected for it.
//! `wiki/spec/language/generics.md#typed-static-parameters`: "Structural
//! callable | `const F: Transform` | Static calls under the contract of the
//! named callable alias."

mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Output, Stdio};

fn fixture(tier: &str) -> String {
    let path = support::repository_root()
        .join("source-rust/application/tests/fixtures/callables")
        .join(tier)
        .join("unit_callable_alias/main.omg");
    std::fs::read_to_string(path).unwrap()
}

/// Run `omega` on a copy of one source file, so nothing the compiler writes
/// lands in the checkout.
fn invoke(source: &str, arguments: &[&str]) -> (Output, String) {
    let scratch = TemporaryDirectory::create("unit-callable-alias", false).unwrap();
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

#[test]
fn a_callable_alias_without_a_result_selects_a_machine_natively_and_in_psi() {
    let (output, report) = invoke(&fixture("run"), &["run", "--both"]);
    assert_eq!(output.status.code(), Some(0), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_machine_with_a_result_does_not_satisfy_a_callable_alias_without_one() {
    // The twins differ only in the machine selected for `F`.
    assert_eq!(
        fixture("run").replace("apply::<expect_one>(1)", "apply::<echo>(1)"),
        fixture("fail")
    );
    let (output, report) = invoke(&fixture("fail"), &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains(
            "`echo` does not satisfy the callback binder of `apply::F`: its result type is not the binder's"
        ),
        "{report}"
    );
    assert!(report.contains("main.omg:9:31"), "{report}");
    assert_eq!(
        &fixture("fail").lines().nth(8).unwrap()[30..41],
        "apply::<ech"
    );
}
